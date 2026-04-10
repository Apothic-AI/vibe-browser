use crate::commands::CommandResponse;
use crate::storage::ConfigManager;
use agent_client_protocol::{self as acp, Agent as _};
use anyhow::{anyhow, bail, Context, Result};
use regex::Regex;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tauri::State;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

const VIBE_AGENT_COMMAND_KEY: &str = "vibe_agent_command";
const VIBE_AGENT_WORKDIR_KEY: &str = "vibe_agent_workdir";
const VIBE_SPEC_MARKDOWN: &str =
    include_str!("../../../../vibe-protocol-website/public/docs/VIBE.md");

#[derive(Clone)]
pub struct VibeState {
    pub config_manager: ConfigManager,
    pub data_dir: PathBuf,
}

impl VibeState {
    pub fn new(config_manager: ConfigManager, data_dir: PathBuf) -> Self {
        Self {
            config_manager,
            data_dir,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibeAgentSettings {
    pub command: String,
    pub workdir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibeNavigationRequest {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryAttempt {
    pub url: String,
    pub ok: bool,
    pub status_code: Option<u16>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedFile {
    pub path: String,
    pub absolute_path: String,
    pub bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLogEntry {
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibeNavigationResult {
    pub source_url: String,
    pub normalized_url: String,
    pub discovered_url: String,
    pub vibe_markdown: String,
    pub discovery_attempts: Vec<DiscoveryAttempt>,
    pub render_dir: String,
    pub index_path: String,
    pub html: String,
    pub generated_files: Vec<GeneratedFile>,
    pub logs: Vec<AgentLogEntry>,
    pub final_message: Option<String>,
    pub stop_reason: String,
    pub fallback_used: bool,
    pub agent_settings: VibeAgentSettings,
}

#[derive(Debug)]
struct DiscoveredVibeDocument {
    normalized_url: String,
    discovered_url: String,
    vibe_markdown: String,
    attempts: Vec<DiscoveryAttempt>,
}

#[derive(Debug, Clone)]
struct RenderSession {
    root_dir: PathBuf,
    context_dir: PathBuf,
    index_path: PathBuf,
}

#[derive(Debug, Default, Clone)]
struct TrackedWrite {
    path: PathBuf,
    bytes: usize,
}

#[derive(Debug)]
struct AgentRunResult {
    logs: Vec<AgentLogEntry>,
    generated_files: Vec<GeneratedFile>,
    assistant_text: String,
    stop_reason: String,
}

#[derive(Clone)]
struct VibeAcpClient {
    allowed_root: PathBuf,
    logs: Arc<Mutex<Vec<AgentLogEntry>>>,
    writes: Arc<Mutex<Vec<TrackedWrite>>>,
    assistant_text: Arc<Mutex<String>>,
}

impl VibeAcpClient {
    fn new(allowed_root: PathBuf) -> Self {
        Self {
            allowed_root,
            logs: Arc::new(Mutex::new(Vec::new())),
            writes: Arc::new(Mutex::new(Vec::new())),
            assistant_text: Arc::new(Mutex::new(String::new())),
        }
    }

    fn push_log(&self, kind: impl Into<String>, message: impl Into<String>) {
        if let Ok(mut logs) = self.logs.lock() {
            logs.push(AgentLogEntry {
                kind: kind.into(),
                message: message.into(),
            });
        }
    }

    fn record_assistant_chunk(&self, message: &str) {
        if let Ok(mut assistant_text) = self.assistant_text.lock() {
            assistant_text.push_str(message);
        }
    }

    fn snapshot_logs(&self) -> Vec<AgentLogEntry> {
        self.logs
            .lock()
            .map(|logs| logs.clone())
            .unwrap_or_default()
    }

    fn snapshot_writes(&self) -> Vec<TrackedWrite> {
        self.writes
            .lock()
            .map(|writes| writes.clone())
            .unwrap_or_default()
    }

    fn snapshot_assistant_text(&self) -> String {
        self.assistant_text
            .lock()
            .map(|text| text.clone())
            .unwrap_or_default()
    }
}

#[async_trait::async_trait(?Send)]
impl acp::Client for VibeAcpClient {
    async fn request_permission(
        &self,
        args: acp::RequestPermissionRequest,
    ) -> acp::Result<acp::RequestPermissionResponse> {
        self.push_log(
            "permission",
            format!("Agent requested permission for session {}", args.session_id),
        );

        let selected =
            args.options
                .into_iter()
                .find_map(|option| match option.kind {
                    acp::PermissionOptionKind::AllowOnce
                    | acp::PermissionOptionKind::AllowAlways => Some(option.option_id),
                    _ => None,
                });

        let outcome = if let Some(option_id) = selected {
            acp::RequestPermissionOutcome::Selected(acp::SelectedPermissionOutcome::new(option_id))
        } else {
            acp::RequestPermissionOutcome::Cancelled
        };

        Ok(acp::RequestPermissionResponse::new(outcome))
    }

    async fn write_text_file(
        &self,
        args: acp::WriteTextFileRequest,
    ) -> acp::Result<acp::WriteTextFileResponse> {
        let path = normalize_absolute_path(&args.path).map_err(|err| {
            acp::Error::invalid_params().data(serde_json::json!({
                "path": args.path.display().to_string(),
                "reason": err.to_string(),
            }))
        })?;

        if !path.starts_with(&self.allowed_root) {
            return Err(acp::Error::invalid_params().data(serde_json::json!({
                "path": path.display().to_string(),
                "reason": "path must stay within the Vibe render directory",
            })));
        }

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|err| acp::Error::internal_error().data(err.to_string()))?;
        }

        tokio::fs::write(&path, &args.content)
            .await
            .map_err(|err| acp::Error::internal_error().data(err.to_string()))?;

        if let Ok(mut writes) = self.writes.lock() {
            writes.push(TrackedWrite {
                path: path.clone(),
                bytes: args.content.len(),
            });
        }

        let relative = path
            .strip_prefix(&self.allowed_root)
            .map(|value| value.display().to_string())
            .unwrap_or_else(|_| path.display().to_string());

        self.push_log("write", format!("Wrote {}", relative));

        Ok(acp::WriteTextFileResponse::default())
    }

    async fn read_text_file(
        &self,
        args: acp::ReadTextFileRequest,
    ) -> acp::Result<acp::ReadTextFileResponse> {
        let path = normalize_absolute_path(&args.path).map_err(|err| {
            acp::Error::invalid_params().data(serde_json::json!({
                "path": args.path.display().to_string(),
                "reason": err.to_string(),
            }))
        })?;

        if !path.starts_with(&self.allowed_root) {
            return Err(acp::Error::invalid_params().data(serde_json::json!({
                "path": path.display().to_string(),
                "reason": "path must stay within the Vibe render directory",
            })));
        }

        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|err| acp::Error::internal_error().data(err.to_string()))?;

        let sliced = slice_text_content(&content, args.line, args.limit);
        self.push_log("read", format!("Read {}", path.display()));
        Ok(acp::ReadTextFileResponse::new(sliced))
    }

    async fn session_notification(&self, args: acp::SessionNotification) -> acp::Result<()> {
        match args.update {
            acp::SessionUpdate::AgentMessageChunk(chunk) => {
                let text = content_block_to_string(&chunk.content);
                self.record_assistant_chunk(&text);
                self.push_log("agent", text);
            }
            acp::SessionUpdate::AgentThoughtChunk(chunk) => {
                self.push_log("thought", content_block_to_string(&chunk.content));
            }
            acp::SessionUpdate::ToolCall(call) => {
                self.push_log("tool", format!("{call:?}"));
            }
            acp::SessionUpdate::ToolCallUpdate(update) => {
                self.push_log("tool-update", format!("{update:?}"));
            }
            acp::SessionUpdate::Plan(plan) => {
                self.push_log("plan", format!("{plan:?}"));
            }
            other => {
                self.push_log("session", format!("{other:?}"));
            }
        }

        Ok(())
    }
}

#[tauri::command]
pub async fn get_vibe_agent_settings(
    state: State<'_, VibeState>,
) -> Result<CommandResponse<VibeAgentSettings>, String> {
    resolve_agent_settings(&state.config_manager)
        .await
        .map(CommandResponse::success)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn set_vibe_agent_settings(
    settings: VibeAgentSettings,
    state: State<'_, VibeState>,
) -> Result<CommandResponse<VibeAgentSettings>, String> {
    if settings.command.trim().is_empty() {
        return Ok(CommandResponse::error(
            "Agent command cannot be empty.".to_string(),
        ));
    }

    state
        .config_manager
        .set_config_value(VIBE_AGENT_COMMAND_KEY, settings.command.trim())
        .await
        .map_err(|err| err.to_string())?;

    match settings
        .workdir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(workdir) => {
            state
                .config_manager
                .set_config_value(VIBE_AGENT_WORKDIR_KEY, workdir)
                .await
                .map_err(|err| err.to_string())?;
        }
        None => {
            let _ = state
                .config_manager
                .delete_config_value(VIBE_AGENT_WORKDIR_KEY)
                .await;
        }
    }

    resolve_agent_settings(&state.config_manager)
        .await
        .map(CommandResponse::success)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn visit_vibe_url(
    request: VibeNavigationRequest,
    state: State<'_, VibeState>,
) -> Result<CommandResponse<VibeNavigationResult>, String> {
    let agent_settings = resolve_agent_settings(&state.config_manager)
        .await
        .map_err(|err| err.to_string())?;

    match visit_vibe_url_inner(&request.url, &agent_settings, &state.data_dir).await {
        Ok(result) => Ok(CommandResponse::success(result)),
        Err(err) => Ok(CommandResponse::error(err.to_string())),
    }
}

async fn visit_vibe_url_inner(
    url_input: &str,
    agent_settings: &VibeAgentSettings,
    data_dir: &Path,
) -> Result<VibeNavigationResult> {
    let discovered = discover_vibe_document(url_input).await?;
    let render_session = prepare_render_session(data_dir, &discovered, agent_settings).await?;
    let prompt = build_vibe_prompt(&discovered, &render_session);
    let mut agent_run = run_acp_render_agent(agent_settings, &render_session, &prompt).await?;

    let mut fallback_used = false;
    if !render_session.index_path.exists() {
        if let Some(html) = extract_html_document(&agent_run.assistant_text) {
            tokio::fs::write(&render_session.index_path, html.as_bytes())
                .await
                .context("failed to write fallback index.html")?;
            agent_run.generated_files.push(GeneratedFile {
                path: "index.html".to_string(),
                absolute_path: render_session.index_path.display().to_string(),
                bytes: html.len(),
            });
            agent_run.logs.push(AgentLogEntry {
                kind: "fallback".to_string(),
                message: "No ACP file write was observed for index.html, so the browser extracted HTML from the final agent message.".to_string(),
            });
            fallback_used = true;
        }
    }

    let html = tokio::fs::read_to_string(&render_session.index_path)
        .await
        .with_context(|| {
            format!(
                "agent finished without producing a renderable index.html at {}",
                render_session.index_path.display()
            )
        })?;

    let final_message = agent_run
        .assistant_text
        .trim()
        .to_string()
        .chars()
        .take(8_000)
        .collect::<String>();
    let final_message = if final_message.is_empty() {
        None
    } else {
        Some(final_message)
    };

    Ok(VibeNavigationResult {
        source_url: url_input.trim().to_string(),
        normalized_url: discovered.normalized_url,
        discovered_url: discovered.discovered_url,
        vibe_markdown: discovered.vibe_markdown,
        discovery_attempts: discovered.attempts,
        render_dir: render_session.root_dir.display().to_string(),
        index_path: render_session.index_path.display().to_string(),
        html,
        generated_files: agent_run.generated_files,
        logs: agent_run.logs,
        final_message,
        stop_reason: agent_run.stop_reason,
        fallback_used,
        agent_settings: agent_settings.clone(),
    })
}

async fn resolve_agent_settings(config_manager: &ConfigManager) -> Result<VibeAgentSettings> {
    let command = config_manager
        .get_config_value(VIBE_AGENT_COMMAND_KEY)
        .await?
        .unwrap_or_else(default_agent_command);

    let workdir = config_manager
        .get_config_value(VIBE_AGENT_WORKDIR_KEY)
        .await?
        .and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
        .or_else(default_agent_workdir);

    Ok(VibeAgentSettings { command, workdir })
}

fn default_agent_command() -> String {
    "opencode acp".to_string()
}

fn default_agent_workdir() -> Option<String> {
    None
}

async fn discover_vibe_document(url_input: &str) -> Result<DiscoveredVibeDocument> {
    let normalized = normalize_input_url(url_input)?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .context("failed to create HTTP client")?;
    let mut attempts = Vec::new();

    for candidate in build_discovery_candidates(&normalized)? {
        let response = client.get(candidate.as_str()).send().await;
        match response {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    let body = response
                        .text()
                        .await
                        .context("failed to read discovered VIBE.md")?;
                    attempts.push(DiscoveryAttempt {
                        url: candidate.to_string(),
                        ok: true,
                        status_code: Some(status.as_u16()),
                        detail: "VIBE.md discovered".to_string(),
                    });

                    return Ok(DiscoveredVibeDocument {
                        normalized_url: normalized.to_string(),
                        discovered_url: candidate.to_string(),
                        vibe_markdown: body,
                        attempts,
                    });
                }

                attempts.push(DiscoveryAttempt {
                    url: candidate.to_string(),
                    ok: false,
                    status_code: Some(status.as_u16()),
                    detail: format!("HTTP {}", status.as_u16()),
                });
            }
            Err(err) => {
                attempts.push(DiscoveryAttempt {
                    url: candidate.to_string(),
                    ok: false,
                    status_code: None,
                    detail: err.to_string(),
                });
            }
        }
    }

    Err(anyhow!(
        "Unable to discover VIBE.md. Tried: {}",
        attempts
            .iter()
            .map(|attempt| attempt.url.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

fn normalize_input_url(url_input: &str) -> Result<Url> {
    let trimmed = url_input.trim();
    if trimmed.is_empty() {
        bail!("Enter a URL to visit.");
    }

    let candidate = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };

    let url = Url::parse(&candidate).with_context(|| format!("invalid URL: {trimmed}"))?;
    if url.scheme() != "http" && url.scheme() != "https" {
        bail!("Only http:// and https:// URLs are supported right now.");
    }
    Ok(url)
}

fn build_discovery_candidates(url: &Url) -> Result<Vec<Url>> {
    if url.path().ends_with("/VIBE.md") || url.path().ends_with("VIBE.md") {
        return Ok(vec![url.clone()]);
    }

    let origin = format!(
        "{}://{}",
        url.scheme(),
        url.host_str()
            .ok_or_else(|| anyhow!("URL is missing a host"))?
    );
    let mut candidates = vec![
        Url::parse(&format!("{origin}/.well-known/VIBE.md"))?,
        Url::parse(&format!("{origin}/VIBE.md"))?,
    ];

    if let Some(port) = url.port() {
        let origin_with_port = format!(
            "{}://{}:{}",
            url.scheme(),
            url.host_str()
                .ok_or_else(|| anyhow!("URL is missing a host"))?,
            port
        );
        candidates = vec![
            Url::parse(&format!("{origin_with_port}/.well-known/VIBE.md"))?,
            Url::parse(&format!("{origin_with_port}/VIBE.md"))?,
        ];
    }

    Ok(candidates)
}

async fn prepare_render_session(
    data_dir: &Path,
    discovered: &DiscoveredVibeDocument,
    agent_settings: &VibeAgentSettings,
) -> Result<RenderSession> {
    let root_dir = data_dir
        .join("vibe-renders")
        .join(uuid::Uuid::new_v4().to_string());
    let context_dir = root_dir.join("_context");
    tokio::fs::create_dir_all(&context_dir)
        .await
        .context("failed to create Vibe render cache directory")?;

    let prompt = build_vibe_prompt(
        discovered,
        &RenderSession {
            root_dir: root_dir.clone(),
            context_dir: context_dir.clone(),
            index_path: root_dir.join("index.html"),
        },
    );

    tokio::fs::write(
        context_dir.join("VIBE.md"),
        discovered.vibe_markdown.as_bytes(),
    )
    .await?;
    tokio::fs::write(
        context_dir.join("VIBE-PROTOCOL-SPEC.md"),
        VIBE_SPEC_MARKDOWN.as_bytes(),
    )
    .await?;
    tokio::fs::write(context_dir.join("PROMPT.md"), prompt.as_bytes()).await?;
    tokio::fs::write(
        context_dir.join("agent-settings.json"),
        serde_json::to_vec_pretty(agent_settings)?.as_slice(),
    )
    .await?;
    tokio::fs::write(
        context_dir.join("discovery-attempts.json"),
        serde_json::to_vec_pretty(&discovered.attempts)?.as_slice(),
    )
    .await?;

    Ok(RenderSession {
        root_dir: root_dir.clone(),
        context_dir,
        index_path: root_dir.join("index.html"),
    })
}

fn build_vibe_prompt(
    discovered: &DiscoveredVibeDocument,
    render_session: &RenderSession,
) -> String {
    format!(
        r#"# Vibe Browser Rendering Job

You are the rendering agent inside Vibe Browser.

Your job is to convert the discovered VIBE.md into a concrete page that this browser can render.

## Required Output Contract

1. You MUST create the main page at this exact absolute path:
   `{index_path}`
2. Use ACP `fs/write_text_file` to save the page and any additional text files.
3. You MAY create extra files under this directory if they help:
   `{root_dir}`
4. Do NOT write outside `{root_dir}`.
5. Make `index.html` self-contained with inline CSS and inline JS whenever possible.
   The browser renders `index.html` directly, so it must work without depending on extra files.
6. You MAY still create additional files for documentation, alternate views, extracted styles, or future use.
7. If the VIBE.md references integrations or endpoints that you do not support, ignore them gracefully and keep rendering the best page you can from the published instructions.
8. When you are done, send a short final assistant message that starts with `RENDER_READY:` and names the main file path.

## Available Context Files

- Discovered VIBE.md copy: `{context_vibe}`
- Embedded spec copy: `{context_spec}`
- This prompt: `{context_prompt}`

## Target URL

- User requested URL: `{requested_url}`
- Discovered VIBE.md URL: `{discovered_url}`

## Vibe Protocol Spec

````md
{vibe_spec}
````

## Discovered VIBE.md

````md
{vibe_markdown}
````
"#,
        index_path = render_session.index_path.display(),
        root_dir = render_session.root_dir.display(),
        context_vibe = render_session.context_dir.join("VIBE.md").display(),
        context_spec = render_session
            .context_dir
            .join("VIBE-PROTOCOL-SPEC.md")
            .display(),
        context_prompt = render_session.context_dir.join("PROMPT.md").display(),
        requested_url = discovered.normalized_url,
        discovered_url = discovered.discovered_url,
        vibe_spec = VIBE_SPEC_MARKDOWN,
        vibe_markdown = discovered.vibe_markdown,
    )
}

async fn run_acp_render_agent(
    agent_settings: &VibeAgentSettings,
    render_session: &RenderSession,
    prompt: &str,
) -> Result<AgentRunResult> {
    let agent_settings = agent_settings.clone();
    let render_session = render_session.clone();
    let prompt = prompt.to_string();

    tokio::task::spawn_blocking(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("failed to create ACP runtime")?;

        runtime.block_on(async move {
            let mut command = build_agent_process(&agent_settings, &render_session)?;
            let mut child = command.spawn().with_context(|| {
                format!(
                    "failed to start ACP agent command: {}",
                    agent_settings.command
                )
            })?;

            let outgoing = child
                .stdin
                .take()
                .context("agent stdin was not available")?
                .compat_write();
            let incoming = child
                .stdout
                .take()
                .context("agent stdout was not available")?
                .compat();

            let client_impl = VibeAcpClient::new(render_session.root_dir.clone());
            let client_snapshot = client_impl.clone();
            let session_cwd = render_session.root_dir.display().to_string();

            let local_set = tokio::task::LocalSet::new();
            let stop_reason = local_set
                .run_until(async move {
                    let (conn, handle_io) =
                        acp::ClientSideConnection::new(client_impl, outgoing, incoming, |future| {
                            tokio::task::spawn_local(future);
                        });

                    tokio::task::spawn_local(handle_io);

                    conn.initialize(
                        acp::InitializeRequest::new(acp::ProtocolVersion::V1)
                            .client_info(
                                acp::Implementation::new("vibe-browser", env!("CARGO_PKG_VERSION"))
                                    .title("Vibe Browser"),
                            )
                            .client_capabilities(
                                acp::ClientCapabilities::default().fs(
                                    acp::FileSystemCapabilities::default()
                                        .read_text_file(true)
                                        .write_text_file(true),
                                ),
                            ),
                    )
                    .await?;

                    let session = conn
                        .new_session(acp::NewSessionRequest::new(session_cwd))
                        .await?;

                    let response = conn
                        .prompt(acp::PromptRequest::new(
                            session.session_id,
                            vec![prompt.into()],
                        ))
                        .await?;

                    Ok::<String, anyhow::Error>(format!("{:?}", response.stop_reason))
                })
                .await?;

            drop(child);

            let generated_files = client_snapshot
                .snapshot_writes()
                .into_iter()
                .map(|write| GeneratedFile {
                    path: write
                        .path
                        .strip_prefix(&render_session.root_dir)
                        .map(|value| value.display().to_string())
                        .unwrap_or_else(|_| write.path.display().to_string()),
                    absolute_path: write.path.display().to_string(),
                    bytes: write.bytes,
                })
                .collect::<Vec<_>>();

            Ok(AgentRunResult {
                logs: client_snapshot.snapshot_logs(),
                generated_files,
                assistant_text: client_snapshot.snapshot_assistant_text(),
                stop_reason,
            })
        })
    })
    .await
    .map_err(|err| anyhow!("ACP render task failed to join: {err}"))?
}

fn build_agent_process(
    agent_settings: &VibeAgentSettings,
    render_session: &RenderSession,
) -> Result<tokio::process::Command> {
    let mut command = if cfg!(target_os = "windows") {
        let mut command = tokio::process::Command::new("cmd");
        command.args(["/C", agent_settings.command.as_str()]);
        command
    } else {
        let mut command = tokio::process::Command::new("sh");
        command.args(["-lc", agent_settings.command.as_str()]);
        command
    };

    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);

    if let Some(workdir) = agent_settings.workdir.as_deref() {
        command.current_dir(workdir);
    } else {
        command.current_dir(&render_session.root_dir);
    }

    Ok(command)
}

fn content_block_to_string(content: &acp::ContentBlock) -> String {
    match content {
        acp::ContentBlock::Text(text) => text.text.clone(),
        acp::ContentBlock::Image(_) => "<image>".to_string(),
        acp::ContentBlock::Audio(_) => "<audio>".to_string(),
        acp::ContentBlock::ResourceLink(link) => link.uri.clone(),
        acp::ContentBlock::Resource(_) => "<resource>".to_string(),
        _ => "<content>".to_string(),
    }
}

fn slice_text_content(content: &str, line: Option<u32>, limit: Option<u32>) -> String {
    let lines = content.lines().collect::<Vec<_>>();
    let start = line.unwrap_or(1).saturating_sub(1) as usize;
    let end = limit
        .map(|value| start.saturating_add(value as usize))
        .unwrap_or(lines.len())
        .min(lines.len());
    lines.get(start..end).unwrap_or(&[]).join("\n")
}

fn normalize_absolute_path(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        bail!("path must be absolute");
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(value) => normalized.push(value),
        }
    }

    Ok(normalized)
}

fn extract_html_document(assistant_text: &str) -> Option<String> {
    let html_fence = Regex::new(r"(?is)```html\s*(?P<html>.*?)```").ok()?;
    if let Some(captures) = html_fence.captures(assistant_text) {
        return captures
            .name("html")
            .map(|value| value.as_str().trim().to_string())
            .filter(|value| !value.is_empty());
    }

    let generic_fence =
        Regex::new(r"(?is)```(?:\w+)?\s*(?P<html><!doctype html.*?|<html.*?>.*)</html>\s*```")
            .ok()?;
    if let Some(captures) = generic_fence.captures(assistant_text) {
        return captures
            .name("html")
            .map(|value| value.as_str().trim().to_string())
            .filter(|value| !value.is_empty());
    }

    let lower = assistant_text.to_lowercase();
    if lower.contains("<html") || lower.contains("<!doctype html") {
        return Some(assistant_text.trim().to_string());
    }

    None
}
