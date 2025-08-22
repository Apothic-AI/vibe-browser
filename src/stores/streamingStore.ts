import { createSignal } from "solid-js";
import { aiService } from "../services/aiService";

// Types matching dyna-solid streaming store interface
export interface StreamingComponentEvent {
  component: string;
  componentName: string;
  placement?: {
    x: number;
    y: number;
    h: number;
  };
  description?: string;
}

export interface StreamingGenerationRequest {
  query: string;
  gridContext?: any;
  targetGridId?: string;
}

export type StreamingGenerationPhase = 'analysis' | 'generation' | 'complete' | null;

export interface StreamingAIStore {
  // State accessors
  isStreaming: () => boolean;
  currentPhase: () => StreamingGenerationPhase;
  components: () => StreamingComponentEvent[];
  totalComponents: () => number;
  currentComponentIndex: () => number;
  estimatedTimeRemaining: () => number;
  
  // Actions
  startGeneration: (request: StreamingGenerationRequest) => Promise<void>;
  stopGeneration: () => void;
  clearComponents: () => void;
  
  // Metrics
  getMetrics: () => {
    startTime: number | null;
    lastEventTime: number | null;
    totalDuration: number | null;
    componentsGenerated: number;
  };
}

/**
 * Create a Tauri-compatible streaming AI store
 * Integrates with the Rust backend via Tauri events
 */
export function createStreamingAIStore(): StreamingAIStore {
  // Core streaming state
  const [isStreaming, setIsStreaming] = createSignal(false);
  const [currentPhase, setCurrentPhase] = createSignal<StreamingGenerationPhase>(null);
  const [components, setComponents] = createSignal<StreamingComponentEvent[]>([]);
  const [totalComponents, setTotalComponents] = createSignal(0);
  const [currentComponentIndex, setCurrentComponentIndex] = createSignal(0);
  const [estimatedTimeRemaining, setEstimatedTimeRemaining] = createSignal(0);
  
  // Metrics
  const [startTime, setStartTime] = createSignal<number | null>(null);
  const [lastEventTime, setLastEventTime] = createSignal<number | null>(null);
  const [totalDuration, setTotalDuration] = createSignal<number | null>(null);
  
  // Cleanup function for event listeners
  let cleanupListeners: (() => void) | null = null;

  const startGeneration = async (request: StreamingGenerationRequest): Promise<void> => {
    console.log('🌊 TauriStreamingStore: Starting component generation');
    
    // Reset state
    setIsStreaming(true);
    setCurrentPhase('analysis');
    setComponents([]);
    setTotalComponents(0);
    setCurrentComponentIndex(0);
    setEstimatedTimeRemaining(0);
    
    const requestStartTime = Date.now();
    setStartTime(requestStartTime);
    setLastEventTime(requestStartTime);
    setTotalDuration(null);

    try {
      // Set up Tauri event listeners first
      cleanupListeners = await aiService.setupStreamingListeners(
        (component: StreamingComponentEvent) => {
          console.log('🌊 TauriStreamingStore: Received component:', component.componentName);
          
          // Add component to our store
          setComponents(prev => [...prev, component]);
          setCurrentComponentIndex(prev => prev + 1);
          setLastEventTime(Date.now());
          
          // Update phase if this is the first component
          if (currentPhase() === 'analysis') {
            setCurrentPhase('generation');
          }
        },
        () => {
          console.log('✅ TauriStreamingStore: Generation complete');
          
          setIsStreaming(false);
          setCurrentPhase('complete');
          setTotalDuration(Date.now() - requestStartTime);
          
          const finalDuration = Date.now() - requestStartTime;
          console.log(`📊 TauriStreaming: Completed in ${finalDuration}ms`);
          console.log(`📊 Generated ${components().length} components`);
        },
        (error: string) => {
          console.error('❌ TauriStreamingStore: Generation error:', error);
          
          setIsStreaming(false);
          setCurrentPhase('complete');
          setTotalDuration(Date.now() - requestStartTime);
        }
      );

      // Convert our request format to the AI service format
      const componentRequest = {
        requirements: request.query,
        component_type: 'ui-component',
        style_framework: 'tailwind',
        grid_context: request.gridContext
      };

      // Start the streaming generation
      await aiService.startStreamingGeneration(componentRequest);
      
    } catch (error) {
      console.error('TauriStreamingStore: Failed to start generation:', error);
      setIsStreaming(false);
      setCurrentPhase('complete');
      throw error;
    }
  };

  const stopGeneration = () => {
    console.log('🛑 TauriStreamingStore: Stopping generation');
    
    setIsStreaming(false);
    setCurrentPhase('complete');
    
    if (cleanupListeners) {
      cleanupListeners();
      cleanupListeners = null;
    }
    
    if (startTime()) {
      setTotalDuration(Date.now() - startTime()!);
    }
  };

  const clearComponents = () => {
    console.log('🧹 TauriStreamingStore: Clearing components');
    
    setComponents([]);
    setTotalComponents(0);
    setCurrentComponentIndex(0);
    setCurrentPhase(null);
    setStartTime(null);
    setLastEventTime(null);
    setTotalDuration(null);
  };

  const getMetrics = () => ({
    startTime: startTime(),
    lastEventTime: lastEventTime(),
    totalDuration: totalDuration(),
    componentsGenerated: components().length
  });

  return {
    // State accessors
    isStreaming,
    currentPhase,
    components,
    totalComponents,
    currentComponentIndex,
    estimatedTimeRemaining,
    
    // Actions
    startGeneration,
    stopGeneration,
    clearComponents,
    
    // Metrics
    getMetrics
  };
}

// Export a singleton instance for convenience
export const streamingStore = createStreamingAIStore();