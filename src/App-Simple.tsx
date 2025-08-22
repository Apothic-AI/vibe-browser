import { createSignal, createEffect, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

// Import simplified components
import SearchOverlay from "./components/SearchOverlay-Simple";
import { SearchInput } from "./components/SearchInput";
import { searchStore } from "./stores/searchStore";

function App() {
  const [greetMsg, setGreetMsg] = createSignal("");
  const [name, setName] = createSignal("");
  const [isInitialized, setIsInitialized] = createSignal(false);

  async function greet() {
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    setGreetMsg(await invoke("greet", { name: name() }));
  }

  // Simple initialization
  createEffect(async () => {
    try {
      console.log("Initializing simplified app...");
      setIsInitialized(true);
    } catch (error) {
      console.error("Failed to initialize:", error);
      setIsInitialized(true);
    }
  });

  // Keyboard shortcut handler
  createEffect(() => {
    const handleKeydown = (e: KeyboardEvent) => {
      // Ctrl/Cmd + K to toggle search overlay
      if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
        e.preventDefault();
        searchStore.toggleOverlay();
      }
    };

    document.addEventListener('keydown', handleKeydown);
    return () => document.removeEventListener('keydown', handleKeydown);
  });

  return (
    <div class="relative min-h-screen bg-gray-900 text-white">
      {/* Main Application Content */}
      <main class="container mx-auto px-4 py-8">
        <div class="text-center mb-8">
          <h1 class="text-4xl font-bold mb-4 bg-gradient-to-r from-blue-400 to-purple-600 bg-clip-text text-transparent">
            Vibe Browser
          </h1>
          <p class="text-gray-400 text-lg mb-6">
            AI-Powered Component Generation Desktop App
          </p>
          
          {/* Quick start section */}
          <div class="bg-gray-800/50 rounded-lg p-6 mb-8 max-w-2xl mx-auto">
            <h2 class="text-xl font-semibold mb-4">Get Started (Basic Mode)</h2>
            <div class="space-y-3 text-gray-300">
              <p>• Press <kbd class="bg-gray-700 px-2 py-1 rounded text-sm">Ctrl/Cmd + K</kbd> to open the search overlay</p>
              <p>• Test the animation and UI patterns</p>
              <p>• Full AI integration coming next</p>
            </div>
          </div>

          {/* Quick action button */}
          <button
            onClick={() => searchStore.toggleOverlay()}
            class="bg-blue-600 hover:bg-blue-700 px-6 py-3 rounded-lg font-semibold transition-colors duration-200 shadow-lg"
          >
            Test Search Overlay
          </button>
        </div>

        {/* Status indicators */}
        <div class="flex justify-center space-x-4 mb-8">
          <div class={`px-3 py-1 rounded-full text-sm ${
            isInitialized() ? 'bg-green-600/20 text-green-400' : 'bg-yellow-600/20 text-yellow-400'
          }`}>
            {isInitialized() ? 'Basic Mode Ready' : 'Initializing...'}
          </div>
        </div>

        {/* Original demo content - kept for debugging */}
        <details class="bg-gray-800/30 rounded-lg p-4 max-w-md mx-auto">
          <summary class="cursor-pointer text-gray-400 hover:text-white">Debug Panel</summary>
          <div class="mt-4 space-y-4">
            <form
              class="space-y-2"
              onSubmit={(e) => {
                e.preventDefault();
                greet();
              }}
            >
              <input
                id="greet-input"
                onChange={(e) => setName(e.currentTarget.value)}
                placeholder="Enter a name..."
                class="w-full px-3 py-2 bg-gray-700 rounded border border-gray-600 focus:border-blue-500 focus:outline-none"
              />
              <button 
                type="submit"
                class="w-full bg-gray-600 hover:bg-gray-500 px-4 py-2 rounded transition-colors"
              >
                Test Tauri Connection
              </button>
            </form>
            <Show when={greetMsg()}>
              <p class="text-green-400 text-sm">{greetMsg()}</p>
            </Show>
          </div>
        </details>
      </main>

      {/* Search Overlay */}
      <Show when={searchStore.isActive()}>
        <SearchOverlay 
          isActive={searchStore.isActive}
          state={searchStore.state}
          stars={searchStore.stars}
          typingStars={searchStore.typingStars}
        />
        
        {/* Search Input - only show when not completed */}
        <Show when={searchStore.state() !== 'completed'}>
          <SearchInput 
            placeholder="Describe the component you want to create..."
            onSubmit={(query) => {
              console.log("Search submitted:", query);
              // For now, just simulate loading and completion
              setTimeout(() => {
                if (searchStore.state() === 'loading') {
                  // Mark as completed after 2 seconds
                  searchStore.resetToInitial();
                }
              }, 2000);
            }}
          />
        </Show>
      </Show>
    </div>
  );
}

export default App;