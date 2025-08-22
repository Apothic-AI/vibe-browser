import { createSignal, createEffect, Show, createMemo } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

// Import full-featured components
import SearchOverlay from "./components/SearchOverlay";
import { SearchInput } from "./components/SearchInput";
import { searchStore } from "./stores/searchStore";
import { streamingStore } from "./stores/streamingStore";
import { aiService } from "./services/aiService";

// Import dyna-solid types for proper integration
import type { AnyGenerationResult } from 'dyna-solid/ai';

function App() {
  const [greetMsg, setGreetMsg] = createSignal("");
  const [name, setName] = createSignal("");
  const [isInitialized, setIsInitialized] = createSignal(false);
  const [aiConfigured, setAiConfigured] = createSignal(false);

  async function greet() {
    setGreetMsg(await invoke("greet", { name: name() }));
  }

  // Initialize the application and configure AI providers
  createEffect(async () => {
    try {
      console.log("Initializing full vibe-browser app...");
      
      // Check available AI providers
      const providers = await aiService.getProviders();
      console.log("Available AI providers:", providers);
      
      // For demo purposes, try to configure a basic provider
      // In production, this would be done through a settings UI
      try {
        await aiService.configureProvider("demo-provider", {
          provider_type: "openrouter",
          model_name: "anthropic/claude-3-sonnet"
          // Note: API key would be configured through secure settings UI
        });
        
        await aiService.setActiveProvider("demo-provider");
        setAiConfigured(true);
        console.log("AI provider configured successfully");
      } catch (error) {
        console.warn("AI provider configuration failed (expected in demo mode):", error);
        // Continue without AI for UI testing
      }
      
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

  // Create a generation result signal that integrates with searchStore
  const generationResult = createMemo((): AnyGenerationResult | undefined => {
    const result = searchStore.searchResult();
    if (!result) return undefined;
    
    // Convert our search result format to dyna-solid format
    if (result.success && result.components.length > 0) {
      // Multi-component result
      return {
        success: true,
        components: result.components.map(component => ({
          component: component.component_code,
          componentName: component.component_name,
          description: component.description,
          placement: { x: 0, y: 0, h: 4 }
        }))
      };
    } else if (result.error) {
      // Error result
      return {
        success: false,
        error: result.error,
        components: []
      };
    }
    
    return undefined;
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
          
          {/* Feature showcase */}
          <div class="bg-gray-800/50 rounded-lg p-6 mb-8 max-w-3xl mx-auto">
            <h2 class="text-xl font-semibold mb-4">Full-Featured Mode</h2>
            <div class="grid md:grid-cols-2 gap-4 text-gray-300">
              <div class="space-y-2">
                <h3 class="font-medium text-blue-400">🚀 Core Features</h3>
                <p class="text-sm">• Press <kbd class="bg-gray-700 px-2 py-1 rounded text-xs">Ctrl/Cmd + K</kbd> to open AI search</p>
                <p class="text-sm">• Real-time component generation</p>
                <p class="text-sm">• GridStack drag-and-drop interface</p>
                <p class="text-sm">• Streaming progress updates</p>
              </div>
              <div class="space-y-2">
                <h3 class="font-medium text-green-400">⚡ AI Integration</h3>
                <p class="text-sm">• Tauri backend integration</p>
                <p class="text-sm">• Multiple AI provider support</p>
                <p class="text-sm">• Component validation & caching</p>
                <p class="text-sm">• Desktop-optimized UI/UX</p>
              </div>
            </div>
          </div>

          {/* Quick action buttons */}
          <div class="flex justify-center space-x-4">
            <button
              onClick={() => searchStore.toggleOverlay()}
              class="bg-blue-600 hover:bg-blue-700 px-6 py-3 rounded-lg font-semibold transition-colors duration-200 shadow-lg"
            >
              Launch AI Search
            </button>
            <button
              onClick={() => streamingStore.clearComponents()}
              class="bg-gray-600 hover:bg-gray-700 px-6 py-3 rounded-lg font-semibold transition-colors duration-200 shadow-lg"
            >
              Clear Components
            </button>
          </div>
        </div>

        {/* Status indicators */}
        <div class="flex justify-center space-x-4 mb-8">
          <div class={`px-3 py-1 rounded-full text-sm ${
            isInitialized() ? 'bg-green-600/20 text-green-400' : 'bg-yellow-600/20 text-yellow-400'
          }`}>
            {isInitialized() ? 'App Ready' : 'Initializing...'}
          </div>
          <div class={`px-3 py-1 rounded-full text-sm ${
            aiConfigured() ? 'bg-green-600/20 text-green-400' : 'bg-orange-600/20 text-orange-400'
          }`}>
            {aiConfigured() ? 'AI Configured' : 'AI Demo Mode'}
          </div>
          <div class={`px-3 py-1 rounded-full text-sm ${
            searchStore.isActive() ? 'bg-blue-600/20 text-blue-400' : 'bg-gray-600/20 text-gray-400'
          }`}>
            {searchStore.isActive() ? 'Search Active' : 'Search Inactive'}
          </div>
        </div>

        {/* Streaming metrics display */}
        <Show when={streamingStore.isStreaming() || streamingStore.components().length > 0}>
          <div class="bg-gray-800/30 rounded-lg p-4 mb-8 max-w-lg mx-auto">
            <h3 class="font-medium text-blue-400 mb-2">Generation Status</h3>
            <div class="space-y-1 text-sm text-gray-300">
              <p>Phase: <span class="text-white">{streamingStore.currentPhase() || 'idle'}</span></p>
              <p>Components: <span class="text-white">{streamingStore.components().length}</span></p>
              <Show when={streamingStore.isStreaming()}>
                <p class="text-blue-400">⚡ Streaming in progress...</p>
              </Show>
            </div>
          </div>
        </Show>

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
          generationResult={generationResult}
          streamingStore={streamingStore}
          stars={searchStore.stars}
          typingStars={searchStore.typingStars}
        />
        
        {/* Search Input - only show when not completed */}
        <Show when={searchStore.state() !== 'completed'}>
          <SearchInput 
            placeholder="Describe the component you want to create..."
            searchQuery={searchStore.searchQuery}
            setSearchQuery={searchStore.setSearchQuery}
            state={searchStore.state}
            startTyping={searchStore.startTyping}
            stopTyping={searchStore.stopTyping}
            generateComponent={searchStore.generateComponent}
            onSubmit={(query) => {
              console.log("Search submitted:", query);
              // The search store handles the actual generation
            }}
          />
        </Show>
      </Show>
    </div>
  );
}

export default App;