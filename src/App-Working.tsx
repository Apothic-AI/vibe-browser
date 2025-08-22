import { createSignal, createEffect, Show, For } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

// Types for Tauri integration
interface ComponentGenerationRequest {
  requirements: string;
  component_type: string;
  style_framework: string;
}

interface ComponentGenerationResponse {
  component_code: string;
  component_name: string;
  description: string;
  dependencies: string[];
  validation_status: 'Valid' | 'Invalid' | 'RequiresReview';
}

interface CommandResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
  timestamp: string;
}

function App() {
  const [greetMsg, setGreetMsg] = createSignal("");
  const [name, setName] = createSignal("");
  
  // AI Integration State
  const [isInitialized, setIsInitialized] = createSignal(false);
  const [aiConfigured, setAiConfigured] = createSignal(false);
  const [isGenerating, setIsGenerating] = createSignal(false);
  const [generationResults, setGenerationResults] = createSignal<ComponentGenerationResponse[]>([]);
  
  // UI State
  const [showSearchOverlay, setShowSearchOverlay] = createSignal(false);
  const [searchQuery, setSearchQuery] = createSignal("");
  const [lastError, setLastError] = createSignal<string | null>(null);
  
  // Stars animation state
  const [stars, setStars] = createSignal<Array<{id: number, top: string, left: string, delay: string}>>([]);

  async function greet() {
    setGreetMsg(await invoke("greet", { name: name() }));
  }

  // Initialize stars for animation
  const initializeStars = () => {
    const newStars: Array<{id: number, top: string, left: string, delay: string}> = [];
    
    for (let i = 0; i < 50; i++) {
      newStars.push({
        id: i,
        top: `${Math.random() * 100}%`,
        left: `${Math.random() * 100}%`,
        delay: `${Math.random() * 3}s`
      });
    }
    
    setStars(newStars);
  };

  // Initialize the application and configure AI providers
  createEffect(async () => {
    try {
      console.log("Initializing vibe-browser app...");
      initializeStars();
      
      // Test basic Tauri communication
      try {
        const testResponse: CommandResponse<string[]> = await invoke('get_ai_providers');
        console.log("AI providers available:", testResponse);
        setAiConfigured(testResponse.success);
      } catch (error) {
        console.warn("AI providers not available (expected in demo):", error);
      }
      
      setIsInitialized(true);
    } catch (error) {
      console.error("Failed to initialize:", error);
      setIsInitialized(true);
    }
  });

  // Keyboard shortcuts
  createEffect(() => {
    const handleKeydown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
        e.preventDefault();
        setShowSearchOverlay(!showSearchOverlay());
        setSearchQuery("");
      }
      
      if (e.key === 'Escape' && showSearchOverlay()) {
        setShowSearchOverlay(false);
        setSearchQuery("");
      }
    };

    document.addEventListener('keydown', handleKeydown);
    return () => document.removeEventListener('keydown', handleKeydown);
  });

  // Component generation function
  const generateComponent = async (requirements: string) => {
    if (!requirements.trim()) return;
    
    setIsGenerating(true);
    setLastError(null);
    
    try {
      console.log("Starting component generation for:", requirements);
      
      const request: ComponentGenerationRequest = {
        requirements,
        component_type: 'ui-component',
        style_framework: 'tailwind'
      };

      const response: CommandResponse<ComponentGenerationResponse> = await invoke('generate_component', {
        request
      });

      if (response.success && response.data) {
        console.log("Component generated successfully:", response.data);
        setGenerationResults(prev => [...prev, response.data!]);
        setShowSearchOverlay(false);
      } else {
        throw new Error(response.error || 'Component generation failed');
      }
    } catch (error) {
      console.error('Component generation failed:', error);
      setLastError(error instanceof Error ? error.message : 'Unknown error occurred');
    } finally {
      setIsGenerating(false);
    }
  };

  // Handle search form submission
  const handleSearchSubmit = (e: Event) => {
    e.preventDefault();
    const query = searchQuery().trim();
    if (query) {
      generateComponent(query);
    }
  };

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
            <h2 class="text-xl font-semibold mb-4">Working Demo Mode</h2>
            <div class="grid md:grid-cols-2 gap-4 text-gray-300">
              <div class="space-y-2">
                <h3 class="font-medium text-blue-400">🚀 Core Features</h3>
                <p class="text-sm">• Press <kbd class="bg-gray-700 px-2 py-1 rounded text-xs">Ctrl/Cmd + K</kbd> to open AI search</p>
                <p class="text-sm">• Tauri backend integration</p>
                <p class="text-sm">• Component generation workflow</p>
                <p class="text-sm">• Desktop-optimized UI/UX</p>
              </div>
              <div class="space-y-2">
                <h3 class="font-medium text-green-400">✅ Working Integration</h3>
                <p class="text-sm">• AI service commands</p>
                <p class="text-sm">• Search overlay interface</p>
                <p class="text-sm">• Component result display</p>
                <p class="text-sm">• Error handling</p>
              </div>
            </div>
          </div>

          {/* Quick action buttons */}
          <div class="flex justify-center space-x-4">
            <button
              onClick={() => setShowSearchOverlay(!showSearchOverlay())}
              class="bg-blue-600 hover:bg-blue-700 px-6 py-3 rounded-lg font-semibold transition-colors duration-200 shadow-lg"
            >
              Launch AI Search
            </button>
            <button
              onClick={() => setGenerationResults([])}
              class="bg-gray-600 hover:bg-gray-700 px-6 py-3 rounded-lg font-semibold transition-colors duration-200 shadow-lg"
            >
              Clear Results
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
            {aiConfigured() ? 'AI Available' : 'AI Demo Mode'}
          </div>
          <div class={`px-3 py-1 rounded-full text-sm ${
            showSearchOverlay() ? 'bg-blue-600/20 text-blue-400' : 'bg-gray-600/20 text-gray-400'
          }`}>
            {showSearchOverlay() ? 'Search Active' : 'Search Inactive'}
          </div>
        </div>

        {/* Generated Components Display */}
        <Show when={generationResults().length > 0}>
          <div class="mb-8">
            <h3 class="text-xl font-semibold mb-4 text-center">Generated Components</h3>
            <div class="grid gap-4 max-w-4xl mx-auto">
              <For each={generationResults()}>
                {(result) => (
                  <div class="bg-gray-800/50 rounded-lg p-6 border border-gray-700">
                    <div class="flex justify-between items-start mb-4">
                      <div>
                        <h4 class="text-lg font-medium text-blue-400">{result.component_name}</h4>
                        <p class="text-gray-400 text-sm">{result.description}</p>
                      </div>
                      <span class={`px-2 py-1 rounded text-xs ${
                        result.validation_status === 'Valid' 
                          ? 'bg-green-600/20 text-green-400'
                          : result.validation_status === 'Invalid'
                          ? 'bg-red-600/20 text-red-400'
                          : 'bg-yellow-600/20 text-yellow-400'
                      }`}>
                        {result.validation_status}
                      </span>
                    </div>
                    
                    <div class="bg-gray-900/50 rounded p-4 font-mono text-sm">
                      <pre class="whitespace-pre-wrap text-gray-300 max-h-64 overflow-y-auto">
                        {result.component_code}
                      </pre>
                    </div>
                    
                    <Show when={result.dependencies.length > 0}>
                      <div class="mt-3">
                        <span class="text-xs text-gray-500">Dependencies: </span>
                        <span class="text-xs text-blue-400">{result.dependencies.join(', ')}</span>
                      </div>
                    </Show>
                  </div>
                )}
              </For>
            </div>
          </div>
        </Show>

        {/* Debug Panel */}
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
      <Show when={showSearchOverlay()}>
        <div class="fixed inset-0 bg-black/80 backdrop-blur-sm z-50 flex items-center justify-center">
          {/* Stars Animation */}
          <div class="absolute inset-0 overflow-hidden">
            <For each={stars()}>
              {(star) => (
                <div
                  class="absolute w-1 h-1 bg-blue-400 rounded-full animate-pulse"
                  style={{
                    left: star.left,
                    top: star.top,
                    "animation-delay": star.delay
                  }}
                />
              )}
            </For>
          </div>

          {/* Search Form */}
          <div class="relative z-10 w-full max-w-lg mx-4">
            <form onSubmit={handleSearchSubmit} class="space-y-4">
              <div>
                <input
                  type="text"
                  value={searchQuery()}
                  onInput={(e) => setSearchQuery(e.currentTarget.value)}
                  placeholder="Describe the component you want to create..."
                  class="w-full px-6 py-4 bg-gray-800/90 border border-gray-600 rounded-lg text-white placeholder-gray-400 focus:border-blue-500 focus:outline-none focus:ring-2 focus:ring-blue-500/20"
                  disabled={isGenerating()}
                />
              </div>
              
              <div class="flex space-x-3">
                <button
                  type="submit"
                  disabled={isGenerating() || !searchQuery().trim()}
                  class="flex-1 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-600 disabled:cursor-not-allowed px-6 py-3 rounded-lg font-semibold transition-colors"
                >
                  {isGenerating() ? 'Generating...' : 'Generate Component'}
                </button>
                
                <button
                  type="button"
                  onClick={() => setShowSearchOverlay(false)}
                  class="px-6 py-3 bg-gray-600 hover:bg-gray-700 rounded-lg font-semibold transition-colors"
                >
                  Cancel
                </button>
              </div>
            </form>
            
            {/* Error Display */}
            <Show when={lastError()}>
              <div class="mt-4 p-4 bg-red-900/50 border border-red-600/50 rounded-lg">
                <p class="text-red-400 text-sm">{lastError()}</p>
              </div>
            </Show>
          </div>
        </div>
      </Show>
    </div>
  );
}

export default App;