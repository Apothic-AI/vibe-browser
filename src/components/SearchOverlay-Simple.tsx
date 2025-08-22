import { createSignal, Show, For, createEffect } from "solid-js";

interface SearchOverlayProps {
  isActive: () => boolean;
  state: () => 'hidden' | 'initial' | 'typing' | 'loading' | 'completed';
  stars: () => Array<{id: number, top: string, left: string, delay: string}>;
  typingStars: () => Array<{id: number, top: string, left: string}>;
}

export default function SearchOverlay(props: SearchOverlayProps) {
  return (
    <>
      {/* Original Search Overlay with animations - always mounted, controlled by CSS */}
      <div 
        class={`search-focus-overlay ${props.isActive() ? 'active' : ''} ${props.state() === 'loading' ? 'full-loading' : ''} ${props.state() === 'completed' ? 'completed' : ''}`}
        style={{
          "background-color": props.isActive() ? "rgba(26, 26, 26, 1)" : "transparent",
          "pointer-events": props.isActive() ? "auto" : "none",
          display: props.isActive() ? "block" : "none"
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
            <p class="text-blue-400 text-xl opacity-80">
              I'm ready...
            </p>
          </div>
        </Show>

        {/* Loading message */}
        <Show when={props.state() === 'loading'}>
          <div class="fixed top-1/2 left-1/2 transform -translate-x-1/2 -translate-y-1/2 text-center z-10">
            <p class="text-blue-400 text-xl opacity-80 animate-pulse">
              Generating component...
            </p>
          </div>
        </Show>

        {/* Completed message */}
        <Show when={props.state() === 'completed'}>
          <div class="fixed top-1/2 left-1/2 transform -translate-x-1/2 -translate-y-1/2 text-center z-10">
            <p class="text-green-400 text-xl opacity-80">
              Component generated! (GridStack integration coming soon)
            </p>
          </div>
        </Show>
      </div>
    </>
  );
}