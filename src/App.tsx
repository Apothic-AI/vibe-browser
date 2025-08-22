import { createSignal, createEffect, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
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

// Browser state types
type BrowserTab = {
  id: string;
  title: string;
  url: string;
  favicon?: string;
  isLoading: boolean;
  content: 'web' | 'ai-component' | 'new-tab';
  componentData?: ComponentGenerationResponse;
};

function App() {
  // Browser state
  const [currentUrl, setCurrentUrl] = createSignal("vibe://new-tab");
  const [isNavigating, setIsNavigating] = createSignal(false);
  const [canGoBack] = createSignal(false);
  const [canGoForward] = createSignal(false);
  const [currentTab, setCurrentTab] = createSignal<BrowserTab>({
    id: 'tab-1',
    title: 'New Tab',
    url: 'vibe://new-tab',
    isLoading: false,
    content: 'new-tab'
  });
  
  // AI Integration State
  const [isInitialized, setIsInitialized] = createSignal(false);
  const [aiConfigured, setAiConfigured] = createSignal(false);
  const [isGenerating, setIsGenerating] = createSignal(false);
  const [lastError, setLastError] = createSignal<string | null>(null);

  // Initialize the application and configure AI providers
  createEffect(async () => {
    try {
      console.log("Initializing vibe-browser app...");
      
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

  // Browser navigation functions
  const navigateBack = () => {
    console.log("Navigate back");
    // TODO: Implement browser history
  };

  const navigateForward = () => {
    console.log("Navigate forward");
    // TODO: Implement browser history
  };

  const refresh = () => {
    setIsNavigating(true);
    console.log("Refreshing page");
    // TODO: Implement page refresh
    setTimeout(() => setIsNavigating(false), 500);
  };

  const navigateToUrl = (url: string) => {
    setIsNavigating(true);
    setCurrentUrl(url);
    
    // Determine content type based on URL
    if (url.startsWith('ai://')) {
      const query = url.replace('ai://', '');
      setCurrentTab({
        ...currentTab(),
        url,
        title: `AI: ${query}`,
        content: 'ai-component',
        isLoading: true
      });
      generateComponent(query);
    } else if (url === 'vibe://new-tab') {
      setCurrentTab({
        ...currentTab(),
        url,
        title: 'New Tab',
        content: 'new-tab',
        isLoading: false
      });
      setIsNavigating(false);
    } else {
      // Regular web navigation
      setCurrentTab({
        ...currentTab(),
        url,
        title: `Loading...`,
        content: 'web',
        isLoading: true
      });
      // TODO: Implement web navigation
      setTimeout(() => {
        setCurrentTab({
          ...currentTab(),
          title: url,
          isLoading: false
        });
        setIsNavigating(false);
      }, 1000);
    }
  };

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
        
        // Update current tab with component data
        setCurrentTab({
          ...currentTab(),
          componentData: response.data,
          isLoading: false,
          title: `AI: ${response.data.component_name}`
        });
      } else {
        throw new Error(response.error || 'Component generation failed');
      }
    } catch (error) {
      console.error('Component generation failed:', error);
      setLastError(error instanceof Error ? error.message : 'Unknown error occurred');
      setCurrentTab({
        ...currentTab(),
        isLoading: false,
        title: 'Error'
      });
    } finally {
      setIsGenerating(false);
      setIsNavigating(false);
    }
  };

  // Handle address bar submission
  const handleAddressSubmit = (e: Event) => {
    e.preventDefault();
    const url = currentUrl().trim();
    if (url) {
      navigateToUrl(url);
    }
  };

  return (
    <div class="h-screen bg-white text-gray-900 flex flex-col">
      {/* Browser Chrome Header */}
      <div class="bg-gray-100 border-b border-gray-300 px-3 py-2 flex items-center space-x-3">
        {/* Navigation Controls */}
        <div class="flex items-center space-x-1">
          <button
            onClick={navigateBack}
            disabled={!canGoBack()}
            class="p-2 rounded hover:bg-gray-200 disabled:opacity-50 disabled:cursor-not-allowed"
            title="Back"
          >
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
            </svg>
          </button>
          
          <button
            onClick={navigateForward}
            disabled={!canGoForward()}
            class="p-2 rounded hover:bg-gray-200 disabled:opacity-50 disabled:cursor-not-allowed"
            title="Forward"
          >
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
            </svg>
          </button>
          
          <button
            onClick={refresh}
            disabled={isNavigating()}
            class="p-2 rounded hover:bg-gray-200 disabled:opacity-50"
            title="Refresh"
          >
            <svg class={`w-4 h-4 ${isNavigating() ? 'animate-spin' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
            </svg>
          </button>
        </div>

        {/* Address Bar */}
        <div class="flex-1 max-w-2xl">
          <form onSubmit={handleAddressSubmit} class="relative">
            <input
              type="text"
              value={currentUrl()}
              onInput={(e) => setCurrentUrl(e.currentTarget.value)}
              placeholder="Search with AI or enter address"
              class="w-full px-4 py-2 bg-white border border-gray-300 rounded-full text-sm focus:outline-none focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
            />
            <Show when={currentTab().isLoading}>
              <div class="absolute right-3 top-1/2 transform -translate-y-1/2">
                <div class="w-4 h-4 border-2 border-blue-500 border-t-transparent rounded-full animate-spin"></div>
              </div>
            </Show>
          </form>
        </div>

        {/* Browser Menu */}
        <div class="flex items-center space-x-2">
          <div class="flex items-center space-x-1 text-xs text-gray-600">
            <Show when={isInitialized()}>
              <span class="bg-green-100 text-green-700 px-2 py-1 rounded">Ready</span>
            </Show>
            <Show when={aiConfigured()}>
              <span class="bg-blue-100 text-blue-700 px-2 py-1 rounded">AI</span>
            </Show>
          </div>
          
          <button class="p-2 rounded hover:bg-gray-200" title="Menu">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 5v.01M12 12v.01M12 19v.01M12 6a1 1 0 110-2 1 1 0 010 2zm0 7a1 1 0 110-2 1 1 0 010 2zm0 7a1 1 0 110-2 1 1 0 010 2z" />
            </svg>
          </button>
        </div>
      </div>

      {/* Tab Bar */}
      <div class="bg-gray-50 border-b border-gray-200 px-3 py-1">
        <div class="flex items-center">
          <div class="bg-white border border-gray-300 border-b-0 px-4 py-2 rounded-t text-sm max-w-xs truncate">
            {currentTab().title}
          </div>
          <button class="ml-2 p-1 rounded hover:bg-gray-200 text-gray-500" title="New tab">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
            </svg>
          </button>
        </div>
      </div>

      {/* Main Content Area */}
      <div class="flex-1 bg-white overflow-hidden">
        <Show when={currentTab().content === 'new-tab'}>
          {/* New Tab Page */}
          <div class="h-full flex flex-col items-center justify-center bg-gray-50 p-8">
            <div class="text-center max-w-2xl">
              <h1 class="text-4xl font-light text-gray-700 mb-4">Vibe Browser</h1>
              <p class="text-gray-500 mb-8">AI-powered web browsing and component generation</p>
              
              <div class="grid grid-cols-1 md:grid-cols-2 gap-6 mb-8">
                <div class="bg-white rounded-lg shadow-sm border p-6">
                  <h3 class="font-semibold text-gray-700 mb-3">🌐 Web Navigation</h3>
                  <p class="text-sm text-gray-600 mb-4">Browse the web like any other browser</p>
                  <input
                    type="text"
                    placeholder="Enter URL (e.g., google.com)"
                    class="w-full px-3 py-2 border border-gray-300 rounded focus:outline-none focus:border-blue-500"
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') {
                        const url = e.currentTarget.value;
                        if (url) navigateToUrl(url);
                      }
                    }}
                  />
                </div>
                
                <div class="bg-white rounded-lg shadow-sm border p-6">
                  <h3 class="font-semibold text-gray-700 mb-3">🤖 AI Components</h3>
                  <p class="text-sm text-gray-600 mb-4">Generate UI components with AI</p>
                  <input
                    type="text"
                    placeholder="Describe a component to create"
                    class="w-full px-3 py-2 border border-gray-300 rounded focus:outline-none focus:border-blue-500"
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') {
                        const query = e.currentTarget.value;
                        if (query) navigateToUrl(`ai://${query}`);
                      }
                    }}
                  />
                </div>
              </div>
              
              <div class="text-xs text-gray-400">
                Try: "ai://create a login form" or "ai://blue button with shadow"
              </div>
            </div>
          </div>
        </Show>

        <Show when={currentTab().content === 'ai-component'}>
          {/* AI Component Display */}
          <div class="h-full overflow-auto p-6 bg-gray-50">
            <Show 
              when={currentTab().componentData} 
              fallback={
                <div class="flex items-center justify-center h-full">
                  <div class="text-center">
                    <div class="w-8 h-8 border-2 border-blue-500 border-t-transparent rounded-full animate-spin mx-auto mb-4"></div>
                    <p class="text-gray-600">Generating component...</p>
                  </div>
                </div>
              }
            >
              {(component) => (
                <div class="max-w-4xl mx-auto">
                  <div class="bg-white rounded-lg shadow-sm border p-6 mb-6">
                    <div class="flex justify-between items-start mb-4">
                      <div>
                        <h2 class="text-2xl font-semibold text-gray-800">{component().component_name}</h2>
                        <p class="text-gray-600 mt-1">{component().description}</p>
                      </div>
                      <span class={`px-3 py-1 rounded-full text-sm ${
                        component().validation_status === 'Valid' 
                          ? 'bg-green-100 text-green-700'
                          : component().validation_status === 'Invalid'
                          ? 'bg-red-100 text-red-700'
                          : 'bg-yellow-100 text-yellow-700'
                      }`}>
                        {component().validation_status}
                      </span>
                    </div>
                    
                    <div class="bg-gray-900 rounded p-4 font-mono text-sm">
                      <pre class="text-green-400 whitespace-pre-wrap max-h-96 overflow-y-auto">
                        {component().component_code}
                      </pre>
                    </div>
                    
                    <Show when={component().dependencies.length > 0}>
                      <div class="mt-4 p-3 bg-blue-50 rounded">
                        <span class="text-sm font-medium text-blue-700">Dependencies: </span>
                        <span class="text-sm text-blue-600">{component().dependencies.join(', ')}</span>
                      </div>
                    </Show>
                  </div>
                </div>
              )}
            </Show>
            
            <Show when={lastError()}>
              <div class="max-w-4xl mx-auto">
                <div class="bg-red-50 border border-red-200 rounded-lg p-4">
                  <p class="text-red-800">{lastError()}</p>
                </div>
              </div>
            </Show>
          </div>
        </Show>

        <Show when={currentTab().content === 'web'}>
          {/* Web Content Placeholder */}
          <div class="h-full flex items-center justify-center bg-gray-100">
            <div class="text-center">
              <div class="w-16 h-16 bg-gray-300 rounded-full flex items-center justify-center mx-auto mb-4">
                <svg class="w-8 h-8 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 12a9 9 0 01-9 9m9-9a9 9 0 00-9-9m9 9H3m9 9v-9m0-9v9" />
                </svg>
              </div>
              <h3 class="text-lg font-medium text-gray-700 mb-2">Web Navigation</h3>
              <p class="text-gray-500 text-sm">This would display web content</p>
              <p class="text-gray-400 text-xs mt-2">Currently showing: {currentTab().url}</p>
            </div>
          </div>
        </Show>
      </div>
    </div>
  );
}

export default App;