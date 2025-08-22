import { createSignal, createEffect } from "solid-js";

// Types
export type SearchState = 'hidden' | 'initial' | 'typing' | 'loading' | 'completed';

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
  
  // Animation state
  const [stars, setStars] = createSignal<Star[]>([]);
  const [typingStars, setTypingStars] = createSignal<TypingStar[]>([]);
  
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

  const generateComponent = async (requirements: string) => {
    if (!requirements.trim()) return;

    setState('loading');
    setSearchQuery(requirements);

    try {
      // Simulate API call
      await new Promise(resolve => setTimeout(resolve, 2000));
      
      setState('completed');
      console.log('Component generation completed (simulated)');
    } catch (error) {
      console.error('Component generation failed:', error);
      setState('completed');
    }
  };

  const resetToInitial = () => {
    setState('initial');
    setSearchQuery('');
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

  return {
    // State
    isActive,
    state,
    searchQuery,
    stars,
    typingStars,
    
    // Actions
    toggleOverlay,
    startTyping,
    stopTyping,
    generateComponent,
    resetToInitial,
    setSearchQuery,
    
    // Utilities
    initializeStars
  };
}

export const searchStore = createSearchStore();