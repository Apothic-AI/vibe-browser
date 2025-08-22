import { createSignal, createEffect } from "solid-js";
import { aiService, type ComponentGenerationResponse } from "../services/aiService";

// Types
export type SearchState = 'hidden' | 'initial' | 'typing' | 'loading' | 'completed';

export interface SearchResult {
  components: ComponentGenerationResponse[];
  success: boolean;
  error?: string;
}

export interface Star {
  id: number;
  top: string;
  left: string;
  delay: string;
}

export interface TypingStar {
  id: number;
  top: string;
  left: string;
}

// Create search store
function createSearchStore() {
  // Core state
  const [isActive, setIsActive] = createSignal(false);
  const [state, setState] = createSignal<SearchState>('hidden');
  const [searchQuery, setSearchQuery] = createSignal('');
  const [searchResult, setSearchResult] = createSignal<SearchResult | undefined>();
  
  // Animation state
  const [stars, setStars] = createSignal<Star[]>([]);
  const [typingStars, setTypingStars] = createSignal<TypingStar[]>([]);
  
  // Streaming state
  const [isStreaming, setIsStreaming] = createSignal(false);
  const [streamingComponents, setStreamingComponents] = createSignal<ComponentGenerationResponse[]>([]);
  
  // Initialize stars for animation
  const initializeStars = () => {
    const newStars: Star[] = [];
    const newTypingStars: TypingStar[] = [];
    
    // Main stars
    for (let i = 0; i < 50; i++) {
      newStars.push({
        id: i,
        top: `${Math.random() * 100}%`,
        left: `${Math.random() * 100}%`,
        delay: `${Math.random() * 3}s`
      });
    }
    
    // Typing stars
    for (let i = 0; i < 20; i++) {
      newTypingStars.push({
        id: i + 50,
        top: `${Math.random() * 100}%`,
        left: `${Math.random() * 100}%`
      });
    }
    
    setStars(newStars);
    setTypingStars(newTypingStars);
  };

  // Initialize stars on first load
  initializeStars();

  // Actions
  const toggleOverlay = () => {
    const wasActive = isActive();
    setIsActive(!wasActive);
    
    if (!wasActive) {
      setState('initial');
      setSearchQuery('');
      setSearchResult(undefined);
      setStreamingComponents([]);
      setIsStreaming(false);
    } else {
      setState('hidden');
    }
  };

  const startTyping = () => {
    if (isActive()) {
      setState('typing');
    }
  };

  const stopTyping = () => {
    if (isActive() && state() === 'typing') {
      setState('initial');
    }
  };

  const generateComponent = async (requirements: string, useStreaming = true) => {
    if (!requirements.trim()) return;

    setState('loading');
    setSearchQuery(requirements);
    setSearchResult(undefined);
    setStreamingComponents([]);

    try {
      if (useStreaming) {
        await startStreamingGeneration(requirements);
      } else {
        await generateSingleComponent(requirements);
      }
    } catch (error) {
      console.error('Component generation failed:', error);
      setSearchResult({
        components: [],
        success: false,
        error: error instanceof Error ? error.message : 'Unknown error occurred'
      });
      setState('completed');
    }
  };

  const startStreamingGeneration = async (requirements: string) => {
    try {
      setIsStreaming(true);
      
      // Set up streaming listeners
      const cleanup = await aiService.setupStreamingListeners(
        (component) => {
          console.log('Received streaming component:', component);
          setStreamingComponents(prev => [...prev, component]);
        },
        () => {
          console.log('Streaming generation completed');
          setIsStreaming(false);
          setState('completed');
          setSearchResult({
            components: streamingComponents(),
            success: true
          });
        },
        (error) => {
          console.error('Streaming generation error:', error);
          setIsStreaming(false);
          setState('completed');
          setSearchResult({
            components: streamingComponents(),
            success: false,
            error
          });
        }
      );

      // Start the streaming generation
      const sessionId = await aiService.startStreamingGeneration({
        requirements,
        component_type: 'ui-component',
        style_framework: 'tailwind'
      });

      console.log('Started streaming generation with session:', sessionId);

      // Store cleanup function for later use
      (window as any).__streamingCleanup = cleanup;

    } catch (error) {
      setIsStreaming(false);
      throw error;
    }
  };

  const generateSingleComponent = async (requirements: string) => {
    try {
      const response = await aiService.generateComponent({
        requirements,
        component_type: 'ui-component',
        style_framework: 'tailwind'
      });

      setSearchResult({
        components: [response],
        success: true
      });
      setState('completed');
    } catch (error) {
      throw error;
    }
  };

  const clearResults = () => {
    setSearchResult(undefined);
    setStreamingComponents([]);
    setSearchQuery('');
    setState('initial');
  };

  const resetToInitial = () => {
    setState('initial');
    setSearchQuery('');
    setSearchResult(undefined);
    setStreamingComponents([]);
    setIsStreaming(false);
  };

  // Keyboard shortcut handler
  createEffect(() => {
    const handleKeydown = (e: KeyboardEvent) => {
      // Escape key to close overlay
      if (e.key === 'Escape' && isActive()) {
        setIsActive(false);
        setState('hidden');
        return;
      }

      // Ctrl/Cmd + K to toggle overlay
      if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
        e.preventDefault();
        toggleOverlay();
        return;
      }

      // Enter to generate component when typing
      if (e.key === 'Enter' && state() === 'typing' && searchQuery().trim()) {
        generateComponent(searchQuery());
        return;
      }
    };

    document.addEventListener('keydown', handleKeydown);
    return () => document.removeEventListener('keydown', handleKeydown);
  });

  // Cleanup streaming listeners on unmount
  createEffect(() => {
    return () => {
      if ((window as any).__streamingCleanup) {
        (window as any).__streamingCleanup();
        delete (window as any).__streamingCleanup;
      }
    };
  });

  return {
    // State
    isActive,
    state,
    searchQuery,
    searchResult,
    stars,
    typingStars,
    isStreaming,
    streamingComponents,
    
    // Actions
    toggleOverlay,
    startTyping,
    stopTyping,
    generateComponent,
    clearResults,
    resetToInitial,
    setSearchQuery,
    
    // Utilities
    initializeStars
  };
}

export const searchStore = createSearchStore();