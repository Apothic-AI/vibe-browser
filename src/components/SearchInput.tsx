import { createSignal, createEffect } from "solid-js";
import { searchStore } from "../stores/searchStore";

interface SearchInputProps {
  placeholder?: string;
  onSubmit?: (query: string) => void;
}

export function SearchInput(props: SearchInputProps) {
  const [inputRef, setInputRef] = createSignal<HTMLInputElement>();
  
  // Focus input when overlay becomes active
  createEffect(() => {
    const active = searchStore.isActive();
    const state = searchStore.state();
    
    if (active && state === 'initial') {
      const input = inputRef();
      if (input) {
        setTimeout(() => input.focus(), 100);
      }
    }
  });

  const handleInput = (e: Event) => {
    const target = e.target as HTMLInputElement;
    const value = target.value;
    
    searchStore.setSearchQuery(value);
    
    if (value.trim()) {
      searchStore.startTyping();
    } else {
      searchStore.stopTyping();
    }
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      const query = searchStore.searchQuery().trim();
      if (query) {
        searchStore.generateComponent(query);
        if (props.onSubmit) {
          props.onSubmit(query);
        }
      }
    }
  };

  const handleFocus = () => {
    if (searchStore.searchQuery().trim()) {
      searchStore.startTyping();
    }
  };

  const handleBlur = () => {
    // Small delay to allow for other interactions
    setTimeout(() => {
      if (!searchStore.searchQuery().trim()) {
        searchStore.stopTyping();
      }
    }, 150);
  };

  return (
    <div class="fixed top-1/2 left-1/2 transform -translate-x-1/2 -translate-y-1/2 w-full max-w-2xl px-8 z-20">
      <div class="relative">
        <input
          ref={setInputRef}
          type="text"
          value={searchStore.searchQuery()}
          onInput={handleInput}
          onKeyDown={handleKeyDown}
          onFocus={handleFocus}
          onBlur={handleBlur}
          placeholder={props.placeholder || "Describe the component you want to create..."}
          class={`
            w-full px-6 py-4 text-lg
            bg-gray-900/95 backdrop-blur-sm
            border border-gray-700/50
            rounded-xl
            text-white placeholder-gray-400
            focus:outline-none focus:ring-2 focus:ring-blue-500/50 focus:border-blue-500/50
            transition-all duration-300
            ${searchStore.state() === 'loading' ? 'animate-pulse cursor-not-allowed' : ''}
          `}
          disabled={searchStore.state() === 'loading'}
        />
        
        {/* Loading indicator */}
        {searchStore.state() === 'loading' && (
          <div class="absolute right-4 top-1/2 transform -translate-y-1/2">
            <div class="w-6 h-6 border-2 border-blue-500/30 border-t-blue-500 rounded-full animate-spin" />
          </div>
        )}
        
        {/* Streaming indicator */}
        {searchStore.isStreaming() && (
          <div class="absolute right-4 top-1/2 transform -translate-y-1/2">
            <div class="flex space-x-1">
              <div class="w-2 h-2 bg-blue-500 rounded-full animate-pulse" />
              <div class="w-2 h-2 bg-blue-500 rounded-full animate-pulse" style="animation-delay: 0.2s" />
              <div class="w-2 h-2 bg-blue-500 rounded-full animate-pulse" style="animation-delay: 0.4s" />
            </div>
          </div>
        )}
      </div>
      
      {/* Helper text */}
      <div class="mt-3 text-center">
        <p class="text-gray-400 text-sm">
          {searchStore.state() === 'loading' ? 
            'Generating your component...' : 
            searchStore.isStreaming() ? 
              'Streaming components in real-time...' :
              'Press Enter to generate • Escape to close'
          }
        </p>
      </div>
    </div>
  );
}