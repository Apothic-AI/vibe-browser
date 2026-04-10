import { For, Show, createMemo, createSignal, onMount } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

interface CommandResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
  timestamp: string;
}

interface VibeAgentSettings {
  command: string;
  workdir?: string | null;
}

interface DiscoveryAttempt {
  url: string;
  ok: boolean;
  status_code?: number | null;
  detail: string;
}

interface GeneratedFile {
  path: string;
  absolute_path: string;
  bytes: number;
}

interface AgentLogEntry {
  kind: string;
  message: string;
}

interface VibeNavigationResult {
  source_url: string;
  normalized_url: string;
  discovered_url: string;
  vibe_markdown: string;
  discovery_attempts: DiscoveryAttempt[];
  render_dir: string;
  index_path: string;
  html: string;
  generated_files: GeneratedFile[];
  logs: AgentLogEntry[];
  final_message?: string | null;
  stop_reason: string;
  fallback_used: boolean;
  agent_settings: VibeAgentSettings;
}

function App() {
  const [url, setUrl] = createSignal("");
  const [isLoading, setIsLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [settingsOpen, setSettingsOpen] = createSignal(false);
  const [agentSettings, setAgentSettings] = createSignal<VibeAgentSettings | null>(null);
  const [settingsCommand, setSettingsCommand] = createSignal("");
  const [settingsWorkdir, setSettingsWorkdir] = createSignal("");
  const [result, setResult] = createSignal<VibeNavigationResult | null>(null);

  const localExampleUrl = createMemo(() => {
    const origin = window.location.origin;
    return origin.startsWith("http") ? origin : null;
  });

  const localYoloCommand = "uv run --project ../../yolo-python yolo-acp";

  const loadSettings = async () => {
    const response = await invoke<CommandResponse<VibeAgentSettings>>("get_vibe_agent_settings");
    if (!response.success || !response.data) {
      throw new Error(response.error || "Failed to load ACP agent settings.");
    }

    setAgentSettings(response.data);
    setSettingsCommand(response.data.command);
    setSettingsWorkdir(response.data.workdir || "");
  };

  onMount(async () => {
    try {
      await loadSettings();
      if (localExampleUrl()) {
        setUrl(localExampleUrl()!);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to initialize Vibe Browser.");
    }
  });

  const visitUrl = async () => {
    const nextUrl = url().trim();
    if (!nextUrl) {
      setError("Enter a URL to visit.");
      return;
    }

    setIsLoading(true);
    setError(null);

    try {
      const response = await invoke<CommandResponse<VibeNavigationResult>>("visit_vibe_url", {
        request: { url: nextUrl },
      });

      if (!response.success || !response.data) {
        throw new Error(response.error || "Navigation failed.");
      }

      setResult(response.data);
      setUrl(response.data.normalized_url);
    } catch (err) {
      setResult(null);
      setError(err instanceof Error ? err.message : "Navigation failed.");
    } finally {
      setIsLoading(false);
    }
  };

  const saveSettings = async () => {
    setError(null);

    try {
      const response = await invoke<CommandResponse<VibeAgentSettings>>("set_vibe_agent_settings", {
        settings: {
          command: settingsCommand().trim(),
          workdir: settingsWorkdir().trim() || null,
        },
      });

      if (!response.success || !response.data) {
        throw new Error(response.error || "Failed to save ACP agent settings.");
      }

      setAgentSettings(response.data);
      setSettingsOpen(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to save ACP agent settings.");
    }
  };

  const handleSubmit = (event: Event) => {
    event.preventDefault();
    void visitUrl();
  };

  return (
    <div class="app-shell">
      <header class="browser-topbar">
        <div class="brand-block">
          <div class="brand-mark">V</div>
          <div>
            <div class="brand-name">Vibe Browser</div>
            <div class="brand-subtitle">discover VIBE.md, ask an ACP agent to render it, show the result</div>
          </div>
        </div>

        <form class="address-bar" onSubmit={handleSubmit}>
          <input
            type="text"
            value={url()}
            onInput={(event) => setUrl(event.currentTarget.value)}
            placeholder="Enter a site URL"
          />
          <button type="submit" class="primary-action" disabled={isLoading()}>
            <Show when={isLoading()} fallback={"Visit"}>
              Rendering…
            </Show>
          </button>
        </form>

        <div class="topbar-actions">
          <button type="button" class="secondary-action" onClick={() => void visitUrl()} disabled={isLoading()}>
            Refresh
          </button>
          <button type="button" class="secondary-action" onClick={() => setSettingsOpen((value) => !value)}>
            ACP Agent
          </button>
        </div>
      </header>

      <Show when={settingsOpen()}>
        <section class="settings-panel">
          <div class="settings-panel__header">
            <div>
              <h2>ACP Agent</h2>
              <p>
                Set any shell command here that starts the ACP process Vibe Browser should use.
                The browser launches it, opens an ACP session, forwards the Vibe prompt plus the
                discovered <code>VIBE.md</code>, and waits for <code>index.html</code>.
              </p>
            </div>
            <button type="button" class="ghost-action" onClick={() => setSettingsOpen(false)}>
              Close
            </button>
          </div>

          <label class="settings-field">
            <span>Agent command</span>
            <input
              type="text"
              value={settingsCommand()}
              onInput={(event) => setSettingsCommand(event.currentTarget.value)}
              placeholder="opencode acp"
            />
            <small class="settings-help">
              Examples: <code>opencode acp</code>, <code>{localYoloCommand}</code>
            </small>
          </label>

          <label class="settings-field">
            <span>Agent working directory</span>
            <input
              type="text"
              value={settingsWorkdir()}
              onInput={(event) => setSettingsWorkdir(event.currentTarget.value)}
              placeholder="Optional"
            />
            <small class="settings-help">
              Leave blank unless the ACP command expects to start from a specific directory.
            </small>
          </label>

          <div class="settings-presets">
            <button type="button" class="secondary-action" onClick={() => setSettingsCommand("opencode acp")}>
              Use opencode acp
            </button>
            <button type="button" class="secondary-action" onClick={() => setSettingsCommand(localYoloCommand)}>
              Use local yolo-acp
            </button>
          </div>

          <div class="settings-actions">
            <button type="button" class="primary-action" onClick={() => void saveSettings()}>
              Save settings
            </button>
          </div>
        </section>
      </Show>

      <Show when={error()}>
        <section class="error-banner">{error()}</section>
      </Show>

      <main class="workspace">
        <section class="preview-column">
          <Show
            when={result()}
            fallback={
              <div class="empty-state">
                <div class="empty-state__card">
                  <div class="eyebrow">MVP Flow</div>
                  <h1>Visit a site and let the agent render it.</h1>
                  <p>
                    The browser runs Vibe discovery first, embeds the protocol spec plus the discovered
                    <code> VIBE.md </code>
                    into an ACP prompt, and expects the agent to write a renderable
                    <code> index.html </code>
                    into the Vibe cache directory.
                  </p>

                  <Show when={localExampleUrl()}>
                    <button
                      type="button"
                      class="primary-action"
                      onClick={() => {
                        setUrl(localExampleUrl()!);
                        void visitUrl();
                      }}
                    >
                      Try local demo
                    </button>
                  </Show>

                  <div class="meta-grid">
                    <div>
                      <span>Discovery</span>
                      <strong>
                        <code>/.well-known/VIBE.md</code> then <code>/VIBE.md</code>
                      </strong>
                    </div>
                    <div>
                      <span>Transport</span>
                      <strong>ACP over stdio</strong>
                    </div>
                    <div>
                      <span>Render target</span>
                      <strong>
                        Agent-written <code>index.html</code>
                      </strong>
                    </div>
                    <div>
                      <span>Current agent</span>
                      <strong>{agentSettings()?.command || "Loading…"}</strong>
                    </div>
                  </div>
                </div>
              </div>
            }
          >
            {(navigation) => (
              <div class="preview-frame-wrap">
                <div class="preview-header">
                  <div>
                    <div class="eyebrow">Rendered Output</div>
                    <h2>{navigation().discovered_url}</h2>
                  </div>
                  <div class="status-row">
                    <span class="status-chip">stop: {navigation().stop_reason}</span>
                    <Show when={navigation().fallback_used}>
                      <span class="status-chip status-chip--warning">fallback html</span>
                    </Show>
                  </div>
                </div>

                <iframe
                  class="preview-frame"
                  sandbox="allow-scripts allow-forms allow-modals"
                  srcdoc={navigation().html}
                  title="Rendered Vibe page"
                />
              </div>
            )}
          </Show>
        </section>

        <aside class="inspector-column">
          <div class="inspector-card">
            <div class="eyebrow">Session</div>
            <h3>Browser state</h3>
            <dl class="detail-list">
              <div>
                <dt>Agent command</dt>
                <dd>{result()?.agent_settings.command || agentSettings()?.command || "Loading…"}</dd>
              </div>
              <div>
                <dt>Render directory</dt>
                <dd>{result()?.render_dir || "No render yet"}</dd>
              </div>
              <div>
                <dt>Main file</dt>
                <dd>{result()?.index_path || "No render yet"}</dd>
              </div>
            </dl>
          </div>

          <Show when={result()}>
            {(navigation) => (
              <>
                <div class="inspector-card">
                  <div class="eyebrow">Discovery</div>
                  <h3>Attempted locations</h3>
                  <ul class="compact-list">
                    <For each={navigation().discovery_attempts}>
                      {(attempt) => (
                        <li classList={{ "compact-list__item--ok": attempt.ok }}>
                          <div>{attempt.url}</div>
                          <small>{attempt.detail}</small>
                        </li>
                      )}
                    </For>
                  </ul>
                </div>

                <div class="inspector-card">
                  <div class="eyebrow">Files</div>
                  <h3>Agent output</h3>
                  <ul class="compact-list">
                    <For each={navigation().generated_files}>
                      {(file) => (
                        <li>
                          <div>{file.path}</div>
                          <small>{file.bytes.toLocaleString()} bytes</small>
                        </li>
                      )}
                    </For>
                  </ul>
                </div>

                <div class="inspector-card">
                  <div class="eyebrow">Agent</div>
                  <h3>Logs</h3>
                  <div class="log-stream">
                    <For each={navigation().logs}>
                      {(entry) => (
                        <div class="log-entry">
                          <span class="log-entry__kind">{entry.kind}</span>
                          <span>{entry.message}</span>
                        </div>
                      )}
                    </For>
                  </div>
                </div>

                <Show when={navigation().final_message}>
                  <div class="inspector-card">
                    <div class="eyebrow">Final message</div>
                    <h3>Agent completion</h3>
                    <pre class="markdown-block">{navigation().final_message}</pre>
                  </div>
                </Show>

                <div class="inspector-card inspector-card--grow">
                  <div class="eyebrow">VIBE.md</div>
                  <h3>Discovered source</h3>
                  <pre class="markdown-block">{navigation().vibe_markdown}</pre>
                </div>
              </>
            )}
          </Show>
        </aside>
      </main>
    </div>
  );
}

export default App;
