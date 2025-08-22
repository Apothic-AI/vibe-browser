import type { Component} from 'solid-js';
import { createSignal, createMemo, createEffect } from 'solid-js';
import { 
  GridStackProvider, 
  GridStackRenderProvider, 
  GridStackRender,
  DynamicRenderer,
  useGridStackContext,
  type ComponentMap
} from 'dyna-solid';
import { 
  createGridContextForLLM,
  type AnyGenerationResult,
  type MultiComponentGenerationResult,
  type AIGenerationResult,
  type StreamingAIStore
} from 'dyna-solid/ai';
import type { GridStackOptions } from 'gridstack';
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// Import dyna-solid specific styles
import 'gridstack/dist/gridstack.min.css';

// Configuration constants for widget operations
const WIDGET_CONFIG = {
  MAX_GRID_INIT_ATTEMPTS: 20,
  GRID_INIT_RETRY_DELAY_MS: 100,
  WIDGET_CONTENT_DELAY_MS: 200,
  DEFAULT_WIDGET_WIDTH: 4,
  DEFAULT_WIDGET_HEIGHT: 4
} as const;

interface DynaGridOverlayProps {
  isActive: () => boolean;
  state: () => 'hidden' | 'initial' | 'typing' | 'loading' | 'completed';
  generationResult?: () => AnyGenerationResult | undefined;
  streamingStore?: StreamingAIStore;
  onGenerated?: (componentData: any) => void;
}

// Helper functions to identify result types
const isMultiComponentResult = (result: AnyGenerationResult): result is MultiComponentGenerationResult => {
  return 'success' in result && 'components' in result;
};

const isSingleComponentResult = (result: AnyGenerationResult): result is AIGenerationResult => {
  return 'componentCode' in result;
};

// Grid Controller component that has access to GridStack context
const GridController: Component<{
  onComponentAdded: (id: string, componentInfo: any) => void;
  initialComponent?: string;
  state: () => 'hidden' | 'initial' | 'typing' | 'loading' | 'completed';
  dynamicComponents: () => Record<string, any>;
  onControllerReady?: (controller: { addWidget: (componentCode: string, placement?: { x: number; y: number; h: number }) => void }) => void;
}> = (props) => {
  const context = useGridStackContext();
  
  // Create reactive signal to track move state changes
  const [moveStateTracker, setMoveStateTracker] = createSignal(0);
  
  // Get move state from overlay manager (desktop equivalent)
  const getMoveEnabled = () => {
    if (typeof window !== 'undefined' && (window as any).overlayManager) {
      return (window as any).overlayManager.getMoveEnabled();
    }
    return false;
  };
  
  // Set up listener for move state changes
  createEffect(() => {
    if (typeof window !== 'undefined' && (window as any).overlayManager) {
      const originalSetMoveEnabled = (window as any).overlayManager.setMoveEnabled;
      (window as any).overlayManager.setMoveEnabled = (enabled: boolean) => {
        originalSetMoveEnabled(enabled);
        setMoveStateTracker(prev => prev + 1); // Trigger effect
      };
    }
  });
  
  const addWidgetToGrid = (componentCode: string, placement?: { x: number; y: number; h: number }) => {
    if (!componentCode.trim() || !context) {
      console.error("Cannot add widget: missing code or context");
      return;
    }

    // Wait for GridStack to be initialized with retry logic
    const tryAddWidget = (attempt = 0) => {
      const gridStack = context.gridStack();
      
      if (!gridStack && attempt < WIDGET_CONFIG.MAX_GRID_INIT_ATTEMPTS) { // Retry for up to 2 seconds
        console.log(`GridStack not ready, retrying... (attempt ${attempt + 1})`);
        setTimeout(() => tryAddWidget(attempt + 1), WIDGET_CONFIG.GRID_INIT_RETRY_DELAY_MS);
        return;
      }
      
      if (!gridStack) {
        console.error(`GridStack failed to initialize after ${WIDGET_CONFIG.MAX_GRID_INIT_ATTEMPTS} attempts`);
        return;
      }

      console.log("GridStack is ready, adding widget...");

      // Create widget with minimal content initially
      const widgetId = context.addWidget((_id) => {
        return {
          x: placement?.x ?? 0,
          y: placement?.y ?? 0,
          w: WIDGET_CONFIG.DEFAULT_WIDGET_WIDTH,
          h: placement?.h ?? WIDGET_CONFIG.DEFAULT_WIDGET_HEIGHT,
          content: '' // Empty content initially - GridStackRender will handle it
        };
      });

      console.log("GridStack addWidget returned:", widgetId);

      // If we got a widget ID back, add it to component data
      if (widgetId) {
        // Use setTimeout to ensure the widget DOM exists before we add component data
        setTimeout(() => {
          console.log("Adding component data for widget:", widgetId);
          // Check if the widget element exists
          // Inline the widget finding logic to avoid import issues
          let widgetElement = null;
          if (gridStack && gridStack.el) {
            widgetElement = gridStack.el.querySelector(`[gs-id="${widgetId}"]`) ||
                           gridStack.el.querySelector(`[data-gs-id="${widgetId}"]`) ||
                           gridStack.el.querySelector(`#${widgetId}`);
          }
          console.log("Widget element exists:", !!widgetElement);
          
          props.onComponentAdded(widgetId, {
            name: "DynamicRenderer",
            props: { jsxString: componentCode },
            jsxCode: componentCode
          });
        }, WIDGET_CONFIG.WIDGET_CONTENT_DELAY_MS);
      }
    };

    tryAddWidget();
  };

  // Track the last component to prevent duplicate adds of the same component
  const [lastAddedComponent, setLastAddedComponent] = createSignal<string>('');
  
  // Expose the addWidgetToGrid function to parent component
  createEffect(() => {
    if (context && props.onControllerReady) {
      props.onControllerReady({
        addWidget: addWidgetToGrid
      });
    }
  });
  
  // Watch for move state changes from overlay manager and apply to grid
  createEffect(() => {
    // Track changes with the moveStateTracker signal
    moveStateTracker();
    
    const grid = context?.gridStack?.();
    if (grid) {
      const moveEnabled = getMoveEnabled();
      
      // Apply move state to GridStack
      grid.enableMove(moveEnabled);
      
      // Update CSS class to control cursor appearance
      const gridEl = grid.el;
      if (gridEl) {
        gridEl.classList.toggle('move-enabled', moveEnabled);
        gridEl.classList.toggle('move-disabled', !moveEnabled);
      }
    }
  });
  
  createEffect(() => {
    // Only add new components - don't re-add when reopening overlay if components already exist
    if (props.initialComponent && context && props.initialComponent !== lastAddedComponent()) {
      const existingComponents = Object.keys(props.dynamicComponents());
      
      // If we already have components and we're not in 'completed' state, skip adding
      // This prevents re-adding components when overlay reopens
      if (existingComponents.length > 0 && props.state() !== 'completed') {
        console.log("🔧 GridController: Skipping re-add on reopen. Existing components:", existingComponents.length, "State:", props.state());
        return;
      }
      
      console.log("Adding component via GridController:", props.initialComponent.slice(0, 50) + '...');
      setLastAddedComponent(props.initialComponent);
      
      // Start trying to add the widget immediately (with retry logic)
      addWidgetToGrid(props.initialComponent);
    }
  });

  return null; // This component doesn't render anything
};


export const DynaGridOverlay: Component<DynaGridOverlayProps> = (props) => {
  // Track generated components
  const [dynamicComponents, setDynamicComponents] = createSignal<Record<string, { 
    name: string; 
    props?: Record<string, unknown>; 
    jsxCode?: string 
  }>>({});

  const [initialComponent, setInitialComponent] = createSignal<string>();

  // Component map for rendering
  const componentMap: ComponentMap = {
    DynamicRenderer,
  };

  // Simple grid options optimized for desktop
  const gridOptions: GridStackOptions = {
    acceptWidgets: true,
    float: true,
    staticGrid: false,
    disableDrag: false,
    margin: 10,
    cellHeight: 70,
    minRow: 6,
    sizeToContent: false, // Disable global sizeToContent - we'll handle it manually after content renders
    column: 24,
    resizable: {
      handles: 'e,se,s,sw,w'
    },
    children: [] // Start empty, let components be added dynamically
  };

  // Handle component added to grid (from GridController)
  const handleComponentAdded = (id: string, componentInfo: any) => {
    console.log("Component added to grid:", id, componentInfo);
    setDynamicComponents(prev => ({
      ...prev,
      [id]: componentInfo
    }));
  };

  // Track the grid controller instance
  const [gridController, setGridController] = createSignal<{ addWidget: (componentCode: string, placement?: { x: number; y: number; h: number }) => void }>();
  
  // Track processed results to avoid duplicates
  const [processedResultId, setProcessedResultId] = createSignal<string>();
  
  // Track streaming components to avoid duplicates
  const [processedStreamingComponents, setProcessedStreamingComponents] = createSignal<Set<string>>(new Set());
  
  // Track if streaming has completed successfully to prevent fallback
  const [streamingCompletedSuccessfully, setStreamingCompletedSuccessfully] = createSignal(false);
  
  // Set up Tauri event listeners for streaming updates
  createEffect(() => {
    let unlisten: (() => void) | undefined;
    
    // Listen for streaming component events from Tauri backend
    const setupTauriListeners = async () => {
      try {
        unlisten = await listen('streaming-component', (event: any) => {
          console.log('🌊 Tauri: Received streaming component event:', event.payload);
          
          const streamingComponent = event.payload;
          const componentKey = `${streamingComponent.componentName}-${Date.now()}`;
          
          const processedSet = processedStreamingComponents();
          if (!processedSet.has(componentKey)) {
            console.log(`🌊 DynaGridOverlay: Adding Tauri streaming component: ${streamingComponent.componentName}`);
            
            const controller = gridController();
            if (controller) {
              // Calculate placement for streaming component
              const existingComponentCount = Object.keys(dynamicComponents()).length;
              const placement = {
                x: streamingComponent.placement?.x ?? (existingComponentCount % 6) * 4,
                y: streamingComponent.placement?.y ?? Math.floor(existingComponentCount / 6) * 4,
                h: streamingComponent.placement?.h ?? 4
              };
              
              // Add component to grid immediately as it's received
              controller.addWidget(streamingComponent.component, placement);
              
              // Mark as processed
              setProcessedStreamingComponents(prev => new Set([...prev, componentKey]));
            } else {
              console.error("GridController not ready for Tauri streaming component:", streamingComponent.componentName);
            }
          }
        });

        // Listen for streaming completion events
        await listen('streaming-complete', (event: any) => {
          console.log('✅ Tauri: Streaming generation complete:', event.payload);
          setStreamingCompletedSuccessfully(true);
        });

        // Listen for streaming error events
        await listen('streaming-error', (event: any) => {
          console.error('❌ Tauri: Streaming generation error:', event.payload);
        });

      } catch (error) {
        console.error('Failed to set up Tauri event listeners:', error);
      }
    };

    setupTauriListeners();

    // Cleanup listeners when component unmounts
    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  });
  
  // Watch for real-time streaming components (highest priority - components appear as they're generated)
  createEffect(() => {
    const streamingStore = props.streamingStore;
    if (!streamingStore) return;
    
    const newComponents = streamingStore.components();
    const processedSet = processedStreamingComponents();
    
    // Check if streaming has completed successfully
    const isCurrentlyStreaming = streamingStore.isStreaming();
    const currentPhase = streamingStore.currentPhase();
    
    // Reset flag when new streaming starts
    if (isCurrentlyStreaming && currentPhase === 'analysis') {
      setStreamingCompletedSuccessfully(false);
    }
    
    // If streaming just completed with components, mark as successful
    if (!isCurrentlyStreaming && currentPhase === 'complete' && newComponents.length > 0) {
      console.log(`✅ STREAMING COMPLETE: Generated ${newComponents.length} components`);
      setStreamingCompletedSuccessfully(true);
    }
    
    // Add new components as they come in from the stream
    newComponents.forEach((streamingComponent, index) => {
      const componentKey = `${streamingComponent.componentName}-${index}`;
      
      if (!processedSet.has(componentKey)) {
        console.log(`🌊 DynaGridOverlay: Adding streaming component: ${streamingComponent.componentName}`);
        
        const controller = gridController();
        if (controller) {
          // Calculate placement for streaming component
          const existingComponentCount = Object.keys(dynamicComponents()).length;
          const placement = {
            x: streamingComponent.placement?.x ?? (existingComponentCount % 6) * 4,
            y: streamingComponent.placement?.y ?? Math.floor(existingComponentCount / 6) * 4,
            h: streamingComponent.placement?.h ?? 4
          };
          
          // Add component to grid immediately as it's received
          controller.addWidget(streamingComponent.component, placement);
          
          // Mark as processed
          setProcessedStreamingComponents(prev => new Set([...prev, componentKey]));
        } else {
          console.error("GridController not ready for streaming component:", streamingComponent.componentName);
        }
      }
    });
  });
  
  // Watch for initial generation result and handle both single and multi-component results (fallback for non-streaming)
  createEffect(() => {
    const result = props.generationResult?.();
    const streamingStore = props.streamingStore;
    
    // Skip if we have streaming store active, no result, or streaming completed successfully
    if (!result || 
        (streamingStore && streamingStore.isStreaming()) || 
        streamingCompletedSuccessfully()) {
      if (streamingCompletedSuccessfully()) {
        console.log("🚫 DynaGridOverlay: Skipping fallback - streaming completed successfully");
      }
      return;
    }

    // Create a unique ID for this result to prevent reprocessing
    const resultId = JSON.stringify(result).slice(0, 100);
    if (processedResultId() === resultId) return;
    setProcessedResultId(resultId);

    console.log("🔍 DynaGridOverlay: Processing fallback generation result:", result);

    if (isMultiComponentResult(result)) {
      // Handle multi-component result
      console.log(`📊 Multi-component result: ${result.components.length} components`);
      
      if (result.success && result.components.length > 0) {
        // Calculate starting position based on existing components to avoid overlap
        const existingComponentCount = Object.keys(dynamicComponents()).length;
        
        // Add each component using the grid controller
        result.components.forEach((component, index) => {
          setTimeout(() => {
            console.log(`🎯 Processing component ${index + 1}/${result.components.length}: ${component.componentName}`);
            
            const controller = gridController();
            if (controller) {
              // Calculate placement considering existing components
              const totalIndex = existingComponentCount + index;
              const placement = {
                x: (totalIndex % 6) * 4,  // 6 columns, 4 units wide each
                y: Math.floor(totalIndex / 6) * 4,  // Stack vertically in rows
                h: 4
              };
              
              controller.addWidget(component.component, placement);
            } else {
              console.error("GridController not ready yet, cannot add component");
            }
          }, index * 500); // Reduced delay to 500ms
        });
      } else {
        console.warn("⚠️ Multi-component result was not successful or has no components");
      }

    } else if (isSingleComponentResult(result)) {
      // Handle single-component result (backwards compatibility)
      if (result.componentCode) {
        console.log("🎯 Adding single component result:", result.componentName);
        
        const controller = gridController();
        if (controller) {
          // Calculate placement considering existing components
          const existingComponentCount = Object.keys(dynamicComponents()).length;
          const placement = {
            x: (existingComponentCount % 3) * 4,
            y: Math.floor(existingComponentCount / 3) * 4,
            h: result.placement?.h ?? 4
          };
          
          controller.addWidget(result.componentCode, placement);
        } else {
          // Fallback to old method if controller not ready
          setInitialComponent(result.componentCode);
        }
      }
    } else {
      console.warn("⚠️ Unknown result type:", result);
    }
  });

  // Create grid context for AI
  const gridContext = createMemo(() => {
    const occupiedSpaces = Object.keys(dynamicComponents()).map((id, index) => ({
      x: (index % 6) * 4,
      y: Math.floor(index / 6) * 4,
      w: 4,
      h: 4,
      id,
      description: `Generated component ${index + 1}`
    }));

    return createGridContextForLLM(24, occupiedSpaces);
  });

  // Component data for GridStackRender
  const componentData = createMemo(() => {
    const data = dynamicComponents();
    console.log("GridStackRender component data:", data);
    return data;
  });

  return (
    <div 
      style={{
        "pointer-events": props.isActive() ? "auto" : "none",
        "background-color": "rgba(26, 26, 26, 1)",
        "position": "absolute",
        "top": "0",
        "left": "0", 
        "width": "100%",
        "height": "100%",
        "z-index": "9999",
        "display": props.isActive() ? "block" : "none"
      }}
    >
        <GridStackProvider 
          initialOptions={gridOptions} 
          componentMap={componentMap}
          enableCommunication={true}
        >
          {/* Grid Controller - handles adding widgets to the grid */}
          <GridController 
            onComponentAdded={handleComponentAdded}
            initialComponent={initialComponent()}
            state={props.state}
            dynamicComponents={dynamicComponents}
            onControllerReady={setGridController}
          />
          

          {/* GridStack Container */}
          <div class="h-full overflow-auto p-8">
            <div class="min-h-full">
              
              <GridStackRenderProvider>
                <GridStackRender 
                  componentMap={componentMap} 
                  componentData={componentData()} 
                />
              </GridStackRenderProvider>
            </div>
          </div>
        </GridStackProvider>
    </div>
  );
};