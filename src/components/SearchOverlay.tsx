import { createSignal, Show, For, createEffect } from "solid-js";
import { DynaGridOverlay } from "./DynaGridOverlay";

import type { AnyGenerationResult } from 'dyna-solid/ai';

interface AIGenerationResult {
  componentCode?: string;
  componentString?: string;
  componentName?: string;
  category?: string;
  description?: string;
  error?: string;
}

import type { StreamingAIStore } from 'dyna-solid/ai';

interface SearchOverlayProps {
  isActive: () => boolean;
  state: () => 'hidden' | 'initial' | 'typing' | 'loading' | 'completed';
  generationResult?: () => AnyGenerationResult | undefined;
  streamingStore?: StreamingAIStore;
  stars: () => Array<{id: number, top: string, left: string, delay: string}>;
  typingStars: () => Array<{id: number, top: string, left: string}>;
}

export default function SearchOverlay(props: SearchOverlayProps) {
  // Track if any components have ever been generated (persists across overlay toggles)
  const [hasEverGeneratedComponents, setHasEverGeneratedComponents] = createSignal(false);
  
  // Helper functions to detect result types
  const isMultiComponentResult = (result: AnyGenerationResult): result is any => {
    return 'success' in result && 'components' in result;
  };
  
  const isSingleComponentResult = (result: AnyGenerationResult): result is any => {
    return 'componentCode' in result;
  };
  
  const hasValidComponents = (result: AnyGenerationResult | undefined): boolean => {
    if (!result) return false;
    
    if (isMultiComponentResult(result)) {
      return result.success && result.components?.length > 0;
    } else if (isSingleComponentResult(result)) {
      return !!result.componentCode;
    }
    
    return false;
  };
  
  // Mark when we first generate components (from traditional or streaming)
  createEffect(() => {
    const result = props.generationResult?.();
    const streamingStore = props.streamingStore;
    
    // Mark components generated from streaming
    if (streamingStore && streamingStore.components().length > 0) {
      setHasEverGeneratedComponents(true);
    }
    
    // Mark components generated from traditional method
    if (hasValidComponents(result) && props.state() === 'completed') {
      setHasEverGeneratedComponents(true);
    }
  });
  
  // Show grid if: currently completed OR we've previously generated components OR streaming is active
  const shouldShowGrid = () => {
    const result = props.generationResult?.();
    const streamingStore = props.streamingStore;
    
    // Show grid if streaming is active or has components
    if (streamingStore && (streamingStore.isStreaming() || streamingStore.components().length > 0)) {
      return true;
    }
    
    // Fallback: show if traditionally completed with valid components
    const currentlyCompleted = hasValidComponents(result) && props.state() === 'completed';
    return currentlyCompleted || hasEverGeneratedComponents();
  };

  return (
    <>
      {/* Original Search Overlay with animations - always mounted, controlled by CSS */}
      <div 
        class={`search-focus-overlay ${props.isActive() ? 'active' : ''} ${props.state() === 'loading' ? 'full-loading' : ''} ${props.state() === 'completed' ? 'completed' : ''}`}
        style={{
          "background-color": props.isActive() && !shouldShowGrid() ? "rgba(26, 26, 26, 1)" : "transparent",
          "pointer-events": props.isActive() && !shouldShowGrid() ? "auto" : "none",
          display: shouldShowGrid() ? "none" : "block"
        }}
      >
        <div class="star-container">
          {/* Main stars */}
          <For each={props.stars()}>
            {(star) => (
              <div
                class="star"
                style={{
                  left: star.left,
                  top: star.top,
                  "animation-delay": star.delay
                }}
              />
            )}
          </For>
          
          {/* Typing stars */}
          <For each={props.typingStars()}>
            {(star) => (
              <div
                class="star typing-star"
                style={{
                  left: star.left,
                  top: star.top
                }}
              />
            )}
          </For>
        </div>
        
        {/* Initial hint message */}
        <Show when={props.state() === 'initial'}>
          <div class="fixed top-1/2 left-1/2 transform -translate-x-1/2 -translate-y-1/2 text-center z-10">
            <p class="text-[#35c7ff] text-xl opacity-80">
              I'm ready...
            </p>
          </div>
        </Show>
      </div>

      {/* Dyna-Solid Grid Overlay - always mounted, visibility controlled internally */}
      <DynaGridOverlay 
        isActive={() => props.isActive() && shouldShowGrid()}
        state={props.state}
        generationResult={props.generationResult}
        streamingStore={props.streamingStore}
        onGenerated={(data) => {
          // Component was generated through the grid interface
          console.log("Component generated:", data);
        }}
      />
    </>
  );
}