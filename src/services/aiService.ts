import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface ComponentGenerationRequest {
  requirements: string;
  component_type: string;
  style_framework: string;
  grid_context?: any;
}

export interface ComponentGenerationResponse {
  component_code: string;
  component_name: string;
  description: string;
  dependencies: string[];
  validation_status: 'Valid' | 'Invalid' | 'RequiresReview';
}

export interface CommandResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
  timestamp: string;
}

export interface AIProviderConfig {
  provider_type: string;
  api_key?: string;
  model_name?: string;
  base_url?: string;
  // Add other provider-specific configs as needed
}

export class AIService {
  private sessionId: string;

  constructor() {
    this.sessionId = `session_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
  }

  /**
   * Generate a single component using Tauri backend
   */
  async generateComponent(request: ComponentGenerationRequest): Promise<ComponentGenerationResponse> {
    try {
      const response: CommandResponse<ComponentGenerationResponse> = await invoke('generate_component', {
        request
      });

      if (response.success && response.data) {
        return response.data;
      } else {
        throw new Error(response.error || 'Failed to generate component');
      }
    } catch (error) {
      console.error('AI Service: Component generation failed:', error);
      throw error;
    }
  }

  /**
   * Start streaming component generation
   */
  async startStreamingGeneration(request: ComponentGenerationRequest): Promise<string> {
    try {
      const response: CommandResponse<string> = await invoke('stream_component_generation', {
        request,
        sessionId: this.sessionId
      });

      if (response.success) {
        return this.sessionId;
      } else {
        throw new Error(response.error || 'Failed to start streaming generation');
      }
    } catch (error) {
      console.error('AI Service: Streaming generation failed:', error);
      throw error;
    }
  }

  /**
   * Validate component code
   */
  async validateComponent(componentCode: string): Promise<boolean> {
    try {
      const response: CommandResponse<{ is_valid: boolean }> = await invoke('validate_component', {
        componentCode
      });

      if (response.success && response.data) {
        return response.data.is_valid;
      } else {
        throw new Error(response.error || 'Failed to validate component');
      }
    } catch (error) {
      console.error('AI Service: Component validation failed:', error);
      return false;
    }
  }

  /**
   * Configure AI provider
   */
  async configureProvider(name: string, config: AIProviderConfig): Promise<void> {
    try {
      const response: CommandResponse<string> = await invoke('configure_ai_provider', {
        name,
        config
      });

      if (!response.success) {
        throw new Error(response.error || 'Failed to configure AI provider');
      }
    } catch (error) {
      console.error('AI Service: Provider configuration failed:', error);
      throw error;
    }
  }

  /**
   * Set active AI provider
   */
  async setActiveProvider(name: string): Promise<void> {
    try {
      const response: CommandResponse<string> = await invoke('set_active_ai_provider', {
        name
      });

      if (!response.success) {
        throw new Error(response.error || 'Failed to set active provider');
      }
    } catch (error) {
      console.error('AI Service: Setting active provider failed:', error);
      throw error;
    }
  }

  /**
   * Get available AI providers
   */
  async getProviders(): Promise<any[]> {
    try {
      const response: CommandResponse<any[]> = await invoke('get_ai_providers');

      if (response.success && response.data) {
        return response.data;
      } else {
        throw new Error(response.error || 'Failed to get providers');
      }
    } catch (error) {
      console.error('AI Service: Getting providers failed:', error);
      return [];
    }
  }

  /**
   * Get cached components
   */
  async getCachedComponents(limit = 20, offset = 0): Promise<any[]> {
    try {
      const response: CommandResponse<any[]> = await invoke('get_cached_components', {
        pagination: { limit, offset }
      });

      if (response.success && response.data) {
        return response.data;
      } else {
        throw new Error(response.error || 'Failed to get cached components');
      }
    } catch (error) {
      console.error('AI Service: Getting cached components failed:', error);
      return [];
    }
  }

  /**
   * Search cached components
   */
  async searchCachedComponents(query: string, limit = 10): Promise<any[]> {
    try {
      const response: CommandResponse<any[]> = await invoke('search_cached_components', {
        search: { query, limit }
      });

      if (response.success && response.data) {
        return response.data;
      } else {
        throw new Error(response.error || 'Failed to search cached components');
      }
    } catch (error) {
      console.error('AI Service: Searching cached components failed:', error);
      return [];
    }
  }

  /**
   * Set up event listeners for streaming updates
   */
  setupStreamingListeners(
    onComponent: (component: any) => void,
    onComplete: () => void,
    onError: (error: string) => void
  ): Promise<() => void> {
    return new Promise(async (resolve, reject) => {
      try {
        const unlistenComponent = await listen('streaming-component', (event: any) => {
          onComponent(event.payload);
        });

        const unlistenComplete = await listen('streaming-complete', (event: any) => {
          onComplete();
        });

        const unlistenError = await listen('streaming-error', (event: any) => {
          onError(event.payload.error || 'Unknown streaming error');
        });

        // Return cleanup function
        resolve(() => {
          unlistenComponent();
          unlistenComplete();
          unlistenError();
        });
      } catch (error) {
        reject(error);
      }
    });
  }

  /**
   * Get session ID for tracking
   */
  getSessionId(): string {
    return this.sessionId;
  }

  /**
   * Generate new session ID
   */
  generateNewSession(): string {
    this.sessionId = `session_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
    return this.sessionId;
  }
}

// Export singleton instance
export const aiService = new AIService();