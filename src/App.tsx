import { For, Show, createEffect, createMemo, createSignal, onMount } from "solid-js";
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
  my_vibes?: string | null;
}

interface AcpModelOption {
  value: string;
  name: string;
  description?: string | null;
}

interface AcpModelSelector {
  config_id: string;
  current_value: string;
  options: AcpModelOption[];
}

interface VibeAgentModelPreference {
  selected_model?: string | null;
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

interface AcpTrafficEntry {
  direction: string;
  event: string;
  summary: string;
  payload: string;
}

type VibeDocumentSource = "published" | "inferred";

interface VibeNavigationResult {
  source_url: string;
  normalized_url: string;
  discovered_url: string;
  vibe_source: VibeDocumentSource;
  vibe_markdown: string;
  discovery_attempts: DiscoveryAttempt[];
  render_dir: string;
  index_path: string;
  html: string;
  generated_files: GeneratedFile[];
  logs: AgentLogEntry[];
  traffic: AcpTrafficEntry[];
  final_message?: string | null;
  stop_reason: string;
  fallback_used: boolean;
  agent_settings: VibeAgentSettings;
  model_selector?: AcpModelSelector | null;
}

type BrowserTabStatus = "idle" | "loading" | "ready" | "error";

interface BrowserTab {
  id: string;
  url: string;
  title: string;
  status: BrowserTabStatus;
  error: string | null;
  result: VibeNavigationResult | null;
}

function createTabId() {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

function displayLabelForUrl(value: string) {
  const trimmed = value.trim();
  if (!trimmed) {
    return "New Tab";
  }

  try {
    const parsed = new URL(trimmed.includes("://") ? trimmed : `https://${trimmed}`);
    const path = parsed.pathname === "/" ? "" : parsed.pathname;
    return `${parsed.host}${path}`;
  } catch {
    return trimmed;
  }
}

function titleForResult(result: VibeNavigationResult) {
  try {
    const parsed = new URL(result.normalized_url);
    return parsed.host;
  } catch {
    return displayLabelForUrl(result.normalized_url);
  }
}

function createBrowserTab(initialUrl = ""): BrowserTab {
  return {
    id: createTabId(),
    url: initialUrl,
    title: displayLabelForUrl(initialUrl),
    status: "idle",
    error: null,
    result: null,
  };
}

function reconcileModelSelector(
  nextSelector: AcpModelSelector | null,
  preferredModel: string,
  previousSelector: AcpModelSelector | null,
): AcpModelSelector | null {
  if (!nextSelector) {
    return null;
  }

  if (!preferredModel) {
    return nextSelector;
  }

  const preferredOption =
    nextSelector.options.find((option) => option.value === preferredModel) ||
    previousSelector?.options.find((option) => option.value === preferredModel);

  if (!preferredOption) {
    return nextSelector;
  }

  if (nextSelector.options.some((option) => option.value === preferredModel)) {
    return {
      ...nextSelector,
      current_value: preferredModel,
    };
  }

  return {
    ...nextSelector,
    current_value: preferredModel,
    options: [preferredOption, ...nextSelector.options],
  };
}

function App() {
  const [isLoading, setIsLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [settingsOpen, setSettingsOpen] = createSignal(false);
  const [detailsOpen, setDetailsOpen] = createSignal(false);
  const [agentSettings, setAgentSettings] = createSignal<VibeAgentSettings | null>(null);
  const [settingsCommand, setSettingsCommand] = createSignal("");
  const [settingsWorkdir, setSettingsWorkdir] = createSignal("");
  const [settingsMyVibes, setSettingsMyVibes] = createSignal("");
  const [modelSelector, setModelSelector] = createSignal<AcpModelSelector | null>(null);
  const [selectedModel, setSelectedModel] = createSignal("");
  const [modelsLoading, setModelsLoading] = createSignal(false);
  const [tabs, setTabs] = createSignal<BrowserTab[]>([]);
  const [activeTabId, setActiveTabId] = createSignal("");
  let modelSelectRef: HTMLSelectElement | undefined;

  const localExampleUrl = createMemo(() => {
    const origin = window.location.origin;
    return origin.startsWith("http") ? origin : null;
  });

  const localYoloCommand = "uv run --project ../../yolo-python yolo-acp";

  const activeTab = createMemo(() => {
    const currentTabs = tabs();
    return currentTabs.find((tab) => tab.id === activeTabId()) || currentTabs[0] || null;
  });

  const currentModelLabel = createMemo(() => {
    const selector = modelSelector();
    const value = selectedModel();
    if (!value) {
      return "Default";
    }

    return selector?.options.find((option) => option.value === value)?.name || value;
  });

  const updateTab = (id: string, updater: (tab: BrowserTab) => BrowserTab) => {
    setTabs((currentTabs) =>
      currentTabs.map((tab) => (tab.id === id ? updater(tab) : tab)),
    );
  };

  const setActiveTabUrl = (value: string) => {
    const tab = activeTab();
    if (!tab) {
      return;
    }

    updateTab(tab.id, (currentTab) => ({
      ...currentTab,
      url: value,
      title: currentTab.result ? currentTab.title : displayLabelForUrl(value),
    }));
  };

  const ensureAtLeastOneTab = (initialUrl = "") => {
    setTabs((currentTabs) => {
      if (currentTabs.length > 0) {
        return currentTabs;
      }

      const nextTab = createBrowserTab(initialUrl);
      setActiveTabId(nextTab.id);
      return [nextTab];
    });
  };

  const createNewTab = (initialUrl = "") => {
    const nextTab = createBrowserTab(initialUrl);
    setTabs((currentTabs) => [...currentTabs, nextTab]);
    setActiveTabId(nextTab.id);
    setError(null);
  };

  const closeTab = (id: string) => {
    setTabs((currentTabs) => {
      if (currentTabs.length <= 1) {
        const replacement = createBrowserTab();
        setActiveTabId(replacement.id);
        return [replacement];
      }

      const index = currentTabs.findIndex((tab) => tab.id === id);
      const nextTabs = currentTabs.filter((tab) => tab.id !== id);

      if (id === activeTabId()) {
        const fallbackTab = nextTabs[index] || nextTabs[index - 1] || nextTabs[0];
        if (fallbackTab) {
          setActiveTabId(fallbackTab.id);
        }
      }

      return nextTabs;
    });
  };

  const loadSettings = async () => {
    const response = await invoke<CommandResponse<VibeAgentSettings>>("get_vibe_agent_settings");
    if (!response.success || !response.data) {
      throw new Error(response.error || "Failed to load ACP agent settings.");
    }

    setAgentSettings(response.data);
    setSettingsCommand(response.data.command);
    setSettingsWorkdir(response.data.workdir || "");
    setSettingsMyVibes(response.data.my_vibes || "");
  };

  const loadModelSelector = async () => {
    setModelsLoading(true);

    try {
      const response = await invoke<CommandResponse<AcpModelSelector | null>>(
        "get_vibe_agent_model_selector",
      );
      if (!response.success) {
        throw new Error(response.error || "Failed to load ACP model options.");
      }

      const selector = reconcileModelSelector(
        response.data || null,
        selectedModel(),
        modelSelector(),
      );
      setModelSelector(selector);
      setSelectedModel((current) => {
        if (!selector) {
          return current;
        }

        if (current && selector.options.some((option) => option.value === current)) {
          return current;
        }

        return selector.current_value || "";
      });
    } finally {
      setModelsLoading(false);
    }
  };

  const persistSelectedModel = async (value: string) => {
    setSelectedModel(value);
    setModelSelector((currentSelector) =>
      reconcileModelSelector(currentSelector, value, currentSelector),
    );
    setError(null);

    try {
      const response = await invoke<CommandResponse<VibeAgentModelPreference>>(
        "set_vibe_agent_model_preference",
        {
          preference: {
            selected_model: value || null,
          },
        },
      );

      if (!response.success) {
        throw new Error(response.error || "Failed to save ACP model preference.");
      }
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to save ACP model preference.",
      );
    }
  };

  onMount(async () => {
    const initialUrl = localExampleUrl() || "";
    ensureAtLeastOneTab(initialUrl);

    try {
      await loadSettings();
      await loadModelSelector();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to initialize Vibe Browser.");
    }
  });

  createEffect(() => {
    const value = selectedModel();
    const selector = modelSelector();

    if (!modelSelectRef) {
      return;
    }

    if (!value) {
      if (selector?.current_value && modelSelectRef.value !== selector.current_value) {
        modelSelectRef.value = selector.current_value;
      }
      return;
    }

    if (modelSelectRef.value !== value) {
      modelSelectRef.value = value;
    }
  });

  const applyReturnedModelSelector = (
    selector: AcpModelSelector | null | undefined,
    requestedModel: string,
  ) => {
    const nextSelector = reconcileModelSelector(
      selector || null,
      requestedModel || selectedModel(),
      modelSelector(),
    );

    if (!nextSelector) {
      if (requestedModel) {
        setSelectedModel(requestedModel);
      }
      return;
    }

    setModelSelector(nextSelector);
    setSelectedModel(requestedModel || selectedModel() || nextSelector.current_value || "");
  };

  const visitActiveTab = async (nextUrlOverride?: string) => {
    const tab = activeTab();
    if (!tab) {
      return;
    }

    const nextUrl = (nextUrlOverride ?? tab.url).trim();
    if (!nextUrl) {
      updateTab(tab.id, (currentTab) => ({
        ...currentTab,
        error: "Enter a URL to visit.",
        status: "error",
      }));
      return;
    }

    const requestedModel = selectedModel();

    setIsLoading(true);
    setError(null);
    updateTab(tab.id, (currentTab) => ({
      ...currentTab,
      url: nextUrl,
      title: displayLabelForUrl(nextUrl),
      status: "loading",
      error: null,
    }));

    try {
      const response = await invoke<CommandResponse<VibeNavigationResult>>("visit_vibe_url", {
        request: {
          url: nextUrl,
          selected_model: requestedModel || null,
        },
      });

      if (!response.success || !response.data) {
        throw new Error(response.error || "Navigation failed.");
      }

      updateTab(tab.id, () => ({
        id: tab.id,
        url: response.data.normalized_url,
        title: titleForResult(response.data),
        status: "ready",
        error: null,
        result: response.data,
      }));
      applyReturnedModelSelector(response.data.model_selector, requestedModel);
    } catch (err) {
      const message = err instanceof Error ? err.message : "Navigation failed.";
      updateTab(tab.id, (currentTab) => ({
        ...currentTab,
        status: "error",
        error: message,
        result: null,
      }));
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
          my_vibes: settingsMyVibes().trim() || null,
        },
      });

      if (!response.success || !response.data) {
        throw new Error(response.error || "Failed to save ACP agent settings.");
      }

      setAgentSettings(response.data);
      await loadModelSelector();
      setSettingsOpen(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to save ACP agent settings.");
    }
  };

  const handleSubmit = (event: Event) => {
    event.preventDefault();
    void visitActiveTab();
  };

  return (
    <div class="app-shell">
      <header class="browser-topbar">
        <div class="brand-block">
          <div class="brand-mark">V</div>
          <div>
            <div class="brand-name">Vibe Browser</div>
            <div class="brand-subtitle">
              discover or infer VIBE.md, ask an ACP agent to render it, show the result
            </div>
          </div>
        </div>

        <form class="address-bar" onSubmit={handleSubmit}>
          <input
            type="text"
            value={activeTab()?.url || ""}
            onInput={(event) => setActiveTabUrl(event.currentTarget.value)}
            placeholder="Enter a site URL"
          />
          <button type="submit" class="primary-action" disabled={isLoading()}>
            <Show when={isLoading()} fallback={"Visit"}>
              Rendering…
            </Show>
          </button>
        </form>

        <div class="topbar-actions">
          <button
            type="button"
            class="secondary-action"
            onClick={() => void visitActiveTab()}
            disabled={isLoading()}
          >
            Refresh
          </button>
          <button
            type="button"
            class="secondary-action"
            onClick={() => setDetailsOpen((value) => !value)}
          >
            <Show when={detailsOpen()} fallback={"Open Details"}>
              Close Details
            </Show>
          </button>
          <button
            type="button"
            class="secondary-action"
            onClick={() => setSettingsOpen((value) => !value)}
          >
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
                published or inferred <code>VIBE.md</code>, and waits for
                <code> index.html</code>.
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

          <Show
            when={modelSelector()}
            fallback={
              <label class="settings-field">
                <span>Model (We recommend OpenRouter/Mercury 2)</span>
                <button
                  type="button"
                  class="secondary-action secondary-action--muted settings-inline-button"
                  disabled={true}
                >
                  <Show when={modelsLoading()} fallback={"No model options"}>
                    Loading models…
                  </Show>
                </button>
                <small class="settings-help">
                  Model options are loaded from the ACP agent&apos;s reported
                  <code> configOptions</code>.
                </small>
              </label>
            }
          >
            {(selector) => (
              <label class="settings-field">
                <span>Model (We recommend OpenRouter/Mercury 2)</span>
                <select
                  ref={(element) => {
                    modelSelectRef = element;
                  }}
                  class="settings-select"
                  value={selectedModel()}
                  onChange={(event) => void persistSelectedModel(event.currentTarget.value)}
                  disabled={isLoading()}
                >
                  <For each={selector().options}>
                    {(option) => <option value={option.value}>{option.name}</option>}
                  </For>
                </select>
                <small class="settings-help">
                  Saved across renders and app restarts. Current selection:
                  <code> {currentModelLabel()}</code>
                </small>
              </label>
            )}
          </Show>

          <label class="settings-field">
            <span>My Vibes</span>
            <textarea
              value={settingsMyVibes()}
              onInput={(event) => setSettingsMyVibes(event.currentTarget.value)}
              placeholder="Add personal rendering instructions for the ACP agent."
            />
            <small class="settings-help">
              Included in the ACP render prompt as a <code>User Instructions</code> section.
              That section explicitly tells the agent to treat your instructions as higher priority
              than the rest of the prompt, the embedded Vibe spec, and the discovered
              <code> VIBE.md</code>.
            </small>
          </label>

          <div class="settings-presets">
            <button
              type="button"
              class="secondary-action"
              onClick={() => setSettingsCommand("opencode acp")}
            >
              Use opencode acp
            </button>
            <button
              type="button"
              class="secondary-action"
              onClick={() => setSettingsCommand(localYoloCommand)}
            >
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
        <aside class="left-tab-panel">
          <div class="tab-panel__header">
            <div>
              <div class="eyebrow">Tabs</div>
              <strong>{tabs().length} open</strong>
            </div>
            <button type="button" class="new-tab-button" onClick={() => createNewTab()}>
              +
            </button>
          </div>

          <div class="tab-list">
            <For each={tabs()}>
              {(tab) => (
                <div class="browser-tab">
                  <button
                    type="button"
                    class="browser-tab__main"
                    classList={{
                      "browser-tab__main--active": activeTabId() === tab.id,
                    }}
                    onClick={() => setActiveTabId(tab.id)}
                  >
                    <div class="browser-tab__title-row">
                      <span
                        class="browser-tab__status"
                        classList={{
                          "browser-tab__status--loading": tab.status === "loading",
                          "browser-tab__status--error": tab.status === "error",
                          "browser-tab__status--ready": tab.status === "ready",
                        }}
                      />
                      <strong class="browser-tab__title">{tab.title}</strong>
                    </div>
                    <span class="browser-tab__url">{tab.url || "Empty tab"}</span>
                  </button>

                  <button
                    type="button"
                    class="browser-tab__close"
                    onClick={(event) => {
                      event.stopPropagation();
                      closeTab(tab.id);
                    }}
                    aria-label={`Close ${tab.title}`}
                  >
                    ×
                  </button>
                </div>
              )}
            </For>
          </div>
        </aside>

        <section class="canvas-column">
          <Show when={activeTab()}>
            {(tab) => (
              <>
                <Show when={tab().status === "ready" && tab().result}>
                  {(navigation) => (
                    <div class="render-stage">
                      <iframe
                        class="preview-frame"
                        sandbox="allow-scripts allow-forms allow-modals"
                        srcdoc={navigation().html}
                        title="Rendered Vibe page"
                      />
                    </div>
                  )}
                </Show>

                <Show when={tab().status === "loading"}>
                  <div class="empty-state">
                    <div class="empty-state__card">
                      <div class="eyebrow">Rendering</div>
                      <h1>{tab().title}</h1>
                      <p>
                        Running Vibe discovery first, then asking the ACP agent to infer a
                        <code> VIBE.md</code> from the target URL when needed, forwarding the
                        render job, and waiting for <code>index.html</code>.
                      </p>
                    </div>
                  </div>
                </Show>

                <Show when={tab().status === "error" && tab().error}>
                  <div class="empty-state">
                    <div class="empty-state__card">
                      <div class="eyebrow">Navigation Error</div>
                      <h1>{tab().title}</h1>
                      <p>{tab().error}</p>
                      <div class="settings-actions">
                        <button
                          type="button"
                          class="primary-action"
                          onClick={() => void visitActiveTab()}
                        >
                          Retry
                        </button>
                      </div>
                    </div>
                  </div>
                </Show>

                <Show when={tab().status === "idle"}>
                  <div class="empty-state">
                    <div class="empty-state__card">
                      <div class="eyebrow">MVP Flow</div>
                      <h1>Visit a site and let the agent render it.</h1>
                      <p>
                        The browser runs Vibe discovery first, and if the site does not publish
                        <code> VIBE.md</code> it asks the ACP agent to infer one from the target
                        URL before embedding the protocol spec plus that request into an ACP
                        prompt. The agent is then expected to write a renderable
                        <code> index.html</code> into the Vibe cache directory.
                      </p>

                      <div class="settings-actions">
                        <Show when={localExampleUrl()}>
                          <button
                            type="button"
                            class="primary-action"
                            onClick={() => {
                              const demoUrl = localExampleUrl();
                              if (!demoUrl) {
                                return;
                              }

                              setActiveTabUrl(demoUrl);
                              void visitActiveTab(demoUrl);
                            }}
                          >
                            Try local demo
                          </button>
                        </Show>

                        <button
                          type="button"
                          class="secondary-action"
                          onClick={() => createNewTab()}
                        >
                          New empty tab
                        </button>
                      </div>

                      <div class="meta-grid">
                        <div>
                          <span>Discovery</span>
                          <strong>
                            <code>/.well-known/VIBE.md</code>, then <code>/VIBE.md</code>, then
                            ask ACP agent to infer from URL
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
                </Show>
              </>
            )}
          </Show>
        </section>

        <Show when={detailsOpen()}>
          <button
            type="button"
            class="drawer-scrim"
            aria-label="Close details panel"
            onClick={() => setDetailsOpen(false)}
          />
        </Show>

        <Show when={detailsOpen()}>
          <aside class="inspector-drawer">
            <div class="inspector-drawer__header">
              <div>
                <div class="eyebrow">Details</div>
                <h2>Session, discovery, and agent notes</h2>
              </div>
              <button type="button" class="ghost-action" onClick={() => setDetailsOpen(false)}>
                Close
              </button>
            </div>

            <div class="inspector-column">
              <div class="inspector-card">
                <div class="eyebrow">Session</div>
                <h3>Browser state</h3>
                <dl class="detail-list">
                  <div>
                    <dt>Agent command</dt>
                    <dd>{activeTab()?.result?.agent_settings.command || agentSettings()?.command || "Loading…"}</dd>
                  </div>
                  <div>
                    <dt>Selected model</dt>
                    <dd>{currentModelLabel()}</dd>
                  </div>
                  <div>
                    <dt>Render directory</dt>
                    <dd>{activeTab()?.result?.render_dir || "No render yet"}</dd>
                  </div>
                  <div>
                    <dt>Main file</dt>
                    <dd>{activeTab()?.result?.index_path || "No render yet"}</dd>
                  </div>
                </dl>
              </div>

              <Show when={activeTab()?.result}>
                {(navigation) => (
                  <>
                    <div class="inspector-card">
                      <div class="eyebrow">Discovery</div>
                      <h3>Attempted locations</h3>
                      <p>
                        Source:{" "}
                        <strong>
                          {navigation().vibe_source === "inferred"
                            ? "Agent-inferred from URL"
                            : "Published VIBE.md"}
                        </strong>
                      </p>
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
                      <div class="eyebrow">ACP</div>
                      <h3>Traffic log</h3>
                      <div class="traffic-stream traffic-stream--drawer">
                        <For each={navigation().traffic}>
                          {(entry) => (
                            <article class="traffic-entry">
                              <div class="traffic-entry__header">
                                <span class="traffic-entry__direction">{entry.direction}</span>
                                <strong>{entry.event}</strong>
                              </div>
                              <p>{entry.summary}</p>
                              <pre class="markdown-block markdown-block--dense">{entry.payload}</pre>
                            </article>
                          )}
                        </For>
                      </div>
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
                      <h3>High-level notes</h3>
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

                    <div class="inspector-card">
                      <div class="eyebrow">Final message</div>
                      <h3>Agent completion</h3>
                      <pre class="markdown-block">
                        {navigation().final_message || "The agent did not send a final text message."}
                      </pre>
                    </div>

                    <div class="inspector-card">
                      <div class="eyebrow">Vibe Document</div>
                      <h3>
                        {navigation().vibe_source === "inferred"
                          ? "Inferred VIBE.md"
                          : "Published VIBE.md"}
                      </h3>
                      <pre class="markdown-block">{navigation().vibe_markdown}</pre>
                    </div>
                  </>
                )}
              </Show>
            </div>
          </aside>
        </Show>
      </main>
    </div>
  );
}

export default App;
