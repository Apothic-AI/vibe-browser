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
use std::time::Duration;
use tauri::State;
use tokio::io::AsyncReadExt;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

const VIBE_AGENT_COMMAND_KEY: &str = "vibe_agent_command";
const VIBE_AGENT_WORKDIR_KEY: &str = "vibe_agent_workdir";
const VIBE_AGENT_MODEL_KEY: &str = "vibe_agent_model";
const VIBE_AGENT_MY_VIBES_KEY: &str = "vibe_agent_my_vibes";
const VIBE_AGENT_LLMS_TXT_TIMEOUT_MS_KEY: &str = "vibe_agent_llms_txt_timeout_ms";
const DEFAULT_RECOMMENDED_MODEL: &str = "openrouter/inception/mercury-2";
const DEFAULT_LLMS_TXT_TIMEOUT_MS: u64 = 250;
// Keep a local copy of the protocol spec so the standalone repo builds outside the monorepo.
const VIBE_SPEC_MARKDOWN: &str = include_str!("../../VIBE-PROTOCOL-SPEC.md");

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
    pub my_vibes: Option<String>,
    pub llms_txt_timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpModelOption {
    pub value: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpModelSelector {
    pub config_id: String,
    pub current_value: String,
    pub options: Vec<AcpModelOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibeNavigationRequest {
    pub url: String,
    pub selected_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibeAgentModelPreference {
    pub selected_model: Option<String>,
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
pub struct AcpTrafficEntry {
    pub direction: String,
    pub event: String,
    pub summary: String,
    pub payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibeNavigationResult {
    pub source_url: String,
    pub normalized_url: String,
    pub discovered_url: String,
    pub vibe_source: VibeDocumentSource,
    pub vibe_markdown: String,
    pub discovery_attempts: Vec<DiscoveryAttempt>,
    pub render_dir: String,
    pub index_path: String,
    pub html: String,
    pub generated_files: Vec<GeneratedFile>,
    pub logs: Vec<AgentLogEntry>,
    pub traffic: Vec<AcpTrafficEntry>,
    pub final_message: Option<String>,
    pub stop_reason: String,
    pub fallback_used: bool,
    pub agent_settings: VibeAgentSettings,
    pub model_selector: Option<AcpModelSelector>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VibeDocumentSource {
    Published,
    Inferred,
}

#[derive(Debug)]
struct DiscoveredVibeDocument {
    normalized_url: String,
    discovered_url: String,
    source: VibeDocumentSource,
    vibe_markdown: String,
    llms_txt: Option<LlmsTextDocument>,
    attempts: Vec<DiscoveryAttempt>,
}

#[derive(Debug, Clone)]
struct LlmsTextDocument {
    url: String,
    content: String,
}

#[derive(Debug, Clone)]
struct RenderSession {
    root_dir: PathBuf,
    context_dir: PathBuf,
    index_path: PathBuf,
    vibe_path: PathBuf,
}

#[derive(Debug, Default, Clone)]
struct TrackedWrite {
    path: PathBuf,
    bytes: usize,
}

#[derive(Debug)]
struct AgentRunResult {
    logs: Vec<AgentLogEntry>,
    traffic: Vec<AcpTrafficEntry>,
    generated_files: Vec<GeneratedFile>,
    assistant_text: String,
    stop_reason: String,
    model_selector: Option<AcpModelSelector>,
}

#[derive(Clone)]
struct VibeAcpClient {
    allowed_root: PathBuf,
    logs: Arc<Mutex<Vec<AgentLogEntry>>>,
    traffic: Arc<Mutex<Vec<AcpTrafficEntry>>>,
    writes: Arc<Mutex<Vec<TrackedWrite>>>,
    assistant_text: Arc<Mutex<String>>,
}

impl VibeAcpClient {
    fn new(allowed_root: PathBuf) -> Self {
        Self {
            allowed_root,
            logs: Arc::new(Mutex::new(Vec::new())),
            traffic: Arc::new(Mutex::new(Vec::new())),
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

    fn push_traffic(
        &self,
        direction: impl Into<String>,
        event: impl Into<String>,
        summary: impl Into<String>,
        payload: impl Into<String>,
    ) {
        if let Ok(mut traffic) = self.traffic.lock() {
            traffic.push(AcpTrafficEntry {
                direction: direction.into(),
                event: event.into(),
                summary: summary.into(),
                payload: payload.into(),
            });
        }
    }

    fn snapshot_logs(&self) -> Vec<AgentLogEntry> {
        self.logs
            .lock()
            .map(|logs| logs.clone())
            .unwrap_or_default()
    }

    fn snapshot_traffic(&self) -> Vec<AcpTrafficEntry> {
        self.traffic
            .lock()
            .map(|traffic| traffic.clone())
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
        self.push_traffic(
            "agent -> browser",
            "request_permission.request",
            format!("Permission request for session {}", args.session_id),
            debug_payload(&args),
        );
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

        self.push_traffic(
            "browser -> agent",
            "request_permission.response",
            "Permission decision returned to agent",
            debug_payload(&outcome),
        );

        Ok(acp::RequestPermissionResponse::new(outcome))
    }

    async fn write_text_file(
        &self,
        args: acp::WriteTextFileRequest,
    ) -> acp::Result<acp::WriteTextFileResponse> {
        self.push_traffic(
            "agent -> browser",
            "fs/write_text_file.request",
            format!("Write request for {}", args.path.display()),
            debug_payload(&args),
        );
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
        self.push_traffic(
            "browser -> agent",
            "fs/write_text_file.response",
            format!("Accepted write for {}", relative),
            "{}",
        );

        Ok(acp::WriteTextFileResponse::default())
    }

    async fn read_text_file(
        &self,
        args: acp::ReadTextFileRequest,
    ) -> acp::Result<acp::ReadTextFileResponse> {
        self.push_traffic(
            "agent -> browser",
            "fs/read_text_file.request",
            format!("Read request for {}", args.path.display()),
            debug_payload(&args),
        );
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
        let response = acp::ReadTextFileResponse::new(sliced);
        self.push_traffic(
            "browser -> agent",
            "fs/read_text_file.response",
            format!("Returned file content for {}", path.display()),
            debug_payload(&response),
        );
        Ok(response)
    }

    async fn session_notification(&self, args: acp::SessionNotification) -> acp::Result<()> {
        self.push_traffic(
            "agent -> browser",
            "session/update",
            "Session notification received",
            debug_payload(&args),
        );
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
pub async fn get_vibe_agent_model_selector(
    state: State<'_, VibeState>,
) -> Result<CommandResponse<Option<AcpModelSelector>>, String> {
    let agent_settings = resolve_agent_settings(&state.config_manager)
        .await
        .map_err(|err| err.to_string())?;
    let preferred_model = resolve_agent_model_preference(&state.config_manager)
        .await
        .map_err(|err| err.to_string())?;

    let selector =
        probe_agent_model_selector(&agent_settings, &state.data_dir, preferred_model.as_deref())
            .await
            .map_err(|err| err.to_string())?;
    let selector = if preferred_model.is_none() {
        apply_default_recommended_model(selector)
    } else {
        selector
    };

    Ok(CommandResponse::success(selector))
}

#[tauri::command]
pub async fn set_vibe_agent_model_preference(
    preference: VibeAgentModelPreference,
    state: State<'_, VibeState>,
) -> Result<CommandResponse<VibeAgentModelPreference>, String> {
    persist_agent_model_preference(&state.config_manager, preference.selected_model.as_deref())
        .await
        .map_err(|err| err.to_string())?;

    let selected_model = resolve_agent_model_preference(&state.config_manager)
        .await
        .map_err(|err| err.to_string())?;

    Ok(CommandResponse::success(VibeAgentModelPreference {
        selected_model,
    }))
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

    if settings.llms_txt_timeout_ms == 0 {
        return Ok(CommandResponse::error(
            "llms.txt timeout must be greater than 0 milliseconds.".to_string(),
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

    match settings
        .my_vibes
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(my_vibes) => {
            state
                .config_manager
                .set_config_value(VIBE_AGENT_MY_VIBES_KEY, my_vibes)
                .await
                .map_err(|err| err.to_string())?;
        }
        None => {
            let _ = state
                .config_manager
                .delete_config_value(VIBE_AGENT_MY_VIBES_KEY)
                .await;
        }
    }

    let llms_txt_timeout_ms = settings.llms_txt_timeout_ms.to_string();
    state
        .config_manager
        .set_config_value(VIBE_AGENT_LLMS_TXT_TIMEOUT_MS_KEY, &llms_txt_timeout_ms)
        .await
        .map_err(|err| err.to_string())?;

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
    let preferred_model = resolve_agent_model_preference(&state.config_manager)
        .await
        .map_err(|err| err.to_string())?;
    let selected_model = request
        .selected_model
        .as_deref()
        .or(preferred_model.as_deref());

    match visit_vibe_url_inner(
        &request.url,
        selected_model,
        &agent_settings,
        &state.data_dir,
    )
    .await
    {
        Ok(result) => {
            if let Some(model) = selected_model {
                persist_agent_model_preference(&state.config_manager, Some(model))
                    .await
                    .map_err(|err| err.to_string())?;
            }

            Ok(CommandResponse::success(result))
        }
        Err(err) => Ok(CommandResponse::error(err.to_string())),
    }
}

async fn visit_vibe_url_inner(
    url_input: &str,
    selected_model: Option<&str>,
    agent_settings: &VibeAgentSettings,
    data_dir: &Path,
) -> Result<VibeNavigationResult> {
    let discovered = discover_vibe_document(
        url_input,
        Duration::from_millis(agent_settings.llms_txt_timeout_ms),
    )
    .await?;
    let render_session = prepare_render_session(data_dir, &discovered, agent_settings).await?;
    let prompt = build_vibe_prompt(&discovered, &render_session, agent_settings);
    let mut agent_run =
        run_acp_render_agent(agent_settings, &render_session, &prompt, selected_model).await?;
    let tool_call_transcript = looks_like_tool_call_transcript(&agent_run.assistant_text);

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

    if !render_session.index_path.exists() && tool_call_transcript {
        bail!(
            "agent described file writes in plain text instead of invoking ACP fs/write_text_file. \
Use a model or adapter that supports ACP tool use, or keep the stricter Vibe Browser render prompt in place so the agent performs the writes instead of narrating them."
        );
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

    let vibe_markdown = match discovered.source {
        VibeDocumentSource::Published => discovered.vibe_markdown.clone(),
        VibeDocumentSource::Inferred => {
            match tokio::fs::read_to_string(&render_session.vibe_path).await {
                Ok(vibe_markdown) => vibe_markdown,
                Err(_) => {
                    agent_run.logs.push(AgentLogEntry {
                    kind: "warning".to_string(),
                    message: format!(
                        "The agent did not write an inferred VIBE.md at {}. Showing the browser fallback instructions instead.",
                        render_session.vibe_path.display()
                    ),
                });
                    discovered.vibe_markdown.clone()
                }
            }
        }
    };

    Ok(VibeNavigationResult {
        source_url: url_input.trim().to_string(),
        normalized_url: discovered.normalized_url,
        discovered_url: discovered.discovered_url,
        vibe_source: discovered.source,
        vibe_markdown,
        discovery_attempts: discovered.attempts,
        render_dir: render_session.root_dir.display().to_string(),
        index_path: render_session.index_path.display().to_string(),
        html,
        generated_files: agent_run.generated_files,
        logs: agent_run.logs,
        traffic: agent_run.traffic,
        final_message,
        stop_reason: agent_run.stop_reason,
        fallback_used,
        agent_settings: agent_settings.clone(),
        model_selector: agent_run.model_selector,
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

    let my_vibes = config_manager
        .get_config_value(VIBE_AGENT_MY_VIBES_KEY)
        .await?
        .and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        });

    Ok(VibeAgentSettings {
        command,
        workdir,
        my_vibes,
        llms_txt_timeout_ms: config_manager
            .get_config_value(VIBE_AGENT_LLMS_TXT_TIMEOUT_MS_KEY)
            .await?
            .and_then(|value| value.trim().parse::<u64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_LLMS_TXT_TIMEOUT_MS),
    })
}

async fn resolve_agent_model_preference(config_manager: &ConfigManager) -> Result<Option<String>> {
    Ok(config_manager
        .get_config_value(VIBE_AGENT_MODEL_KEY)
        .await?
        .and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }))
}

async fn persist_agent_model_preference(
    config_manager: &ConfigManager,
    selected_model: Option<&str>,
) -> Result<()> {
    match selected_model
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(selected_model) => {
            config_manager
                .set_config_value(VIBE_AGENT_MODEL_KEY, selected_model)
                .await?
        }
        None => {
            let _ = config_manager
                .delete_config_value(VIBE_AGENT_MODEL_KEY)
                .await?;
        }
    }

    Ok(())
}

fn default_agent_command() -> String {
    "opencode acp".to_string()
}

fn default_agent_workdir() -> Option<String> {
    None
}

async fn discover_vibe_document(
    url_input: &str,
    llms_txt_timeout: Duration,
) -> Result<DiscoveredVibeDocument> {
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
                        source: VibeDocumentSource::Published,
                        vibe_markdown: body,
                        llms_txt: None,
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

    let llms_txt =
        fetch_optional_llms_txt(&client, &normalized, llms_txt_timeout, &mut attempts).await?;

    Ok(build_agent_inference_document(
        &normalized,
        attempts,
        llms_txt,
    )?)
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

fn build_inference_target(url: &Url) -> Result<Url> {
    if !(url.path().ends_with("/VIBE.md") || url.path().ends_with("VIBE.md")) {
        return Ok(url.clone());
    }

    let mut target = url.clone();
    target.set_query(None);
    target.set_fragment(None);

    let next_path = match target.path() {
        "/VIBE.md" | "/.well-known/VIBE.md" => "/".to_string(),
        path => {
            let trimmed = path.trim_end_matches('/');
            if let Some((parent, _)) = trimmed.rsplit_once('/') {
                if parent.is_empty() {
                    "/".to_string()
                } else {
                    parent.to_string()
                }
            } else {
                "/".to_string()
            }
        }
    };

    target.set_path(&next_path);
    Ok(target)
}

fn build_llms_txt_candidate(url: &Url) -> Result<Url> {
    let mut candidate = url.clone();
    candidate.set_path("/llms.txt");
    candidate.set_query(None);
    candidate.set_fragment(None);
    Ok(candidate)
}

async fn fetch_optional_llms_txt(
    client: &reqwest::Client,
    normalized: &Url,
    timeout: Duration,
    attempts: &mut Vec<DiscoveryAttempt>,
) -> Result<Option<LlmsTextDocument>> {
    let candidate = build_llms_txt_candidate(normalized)?;
    let response = client.get(candidate.as_str()).timeout(timeout).send().await;

    match response {
        Ok(response) => {
            let status = response.status();
            if status.is_success() {
                let body = response
                    .text()
                    .await
                    .context("failed to read discovered llms.txt")?;
                attempts.push(DiscoveryAttempt {
                    url: candidate.to_string(),
                    ok: true,
                    status_code: Some(status.as_u16()),
                    detail: "Fetched llms.txt companion".to_string(),
                });

                Ok(Some(LlmsTextDocument {
                    url: candidate.to_string(),
                    content: body,
                }))
            } else {
                attempts.push(DiscoveryAttempt {
                    url: candidate.to_string(),
                    ok: false,
                    status_code: Some(status.as_u16()),
                    detail: format!("HTTP {}", status.as_u16()),
                });
                Ok(None)
            }
        }
        Err(err) => {
            let detail = if err.is_timeout() {
                format!(
                    "Timed out after {} ms while fetching llms.txt",
                    timeout.as_millis()
                )
            } else {
                err.to_string()
            };
            attempts.push(DiscoveryAttempt {
                url: candidate.to_string(),
                ok: false,
                status_code: None,
                detail,
            });
            Ok(None)
        }
    }
}

fn build_agent_inference_document(
    normalized: &Url,
    mut attempts: Vec<DiscoveryAttempt>,
    llms_txt: Option<LlmsTextDocument>,
) -> Result<DiscoveredVibeDocument> {
    let inference_url = build_inference_target(normalized)?;
    attempts.push(DiscoveryAttempt {
        url: inference_url.to_string(),
        ok: true,
        status_code: None,
        detail: "No published VIBE.md was found. The ACP agent must infer one from this URL."
            .to_string(),
    });

    Ok(DiscoveredVibeDocument {
        normalized_url: normalized.to_string(),
        discovered_url: inference_url.to_string(),
        source: VibeDocumentSource::Inferred,
        vibe_markdown: build_inference_request_markdown(&inference_url, &attempts),
        llms_txt,
        attempts,
    })
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
    let index_path = root_dir.join("index.html");
    let vibe_path = root_dir.join("VIBE.md");
    tokio::fs::create_dir_all(&context_dir)
        .await
        .context("failed to create Vibe render cache directory")?;

    let prompt = build_vibe_prompt(
        discovered,
        &RenderSession {
            root_dir: root_dir.clone(),
            context_dir: context_dir.clone(),
            index_path: index_path.clone(),
            vibe_path: vibe_path.clone(),
        },
        agent_settings,
    );

    if discovered.source == VibeDocumentSource::Published {
        tokio::fs::write(
            context_dir.join("VIBE.md"),
            discovered.vibe_markdown.as_bytes(),
        )
        .await?;
    }
    if let Some(llms_txt) = &discovered.llms_txt {
        tokio::fs::write(context_dir.join("llms.txt"), llms_txt.content.as_bytes()).await?;
    }
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
        root_dir,
        context_dir,
        index_path,
        vibe_path,
    })
}

fn build_vibe_prompt(
    discovered: &DiscoveredVibeDocument,
    render_session: &RenderSession,
    agent_settings: &VibeAgentSettings,
) -> String {
    let user_instructions_section =
        format_user_instructions_section(agent_settings.my_vibes.as_deref());
    let source_specific_section = format_source_specific_section(discovered, render_session);
    let available_context_section = format_available_context_section(discovered, render_session);
    let vibe_document_section = format_vibe_document_section(discovered);
    let llms_txt_section = format_llms_txt_section(discovered);
    format!(
        r#"# Vibe Browser Rendering Job

You are the rendering agent inside Vibe Browser.
 
Your job is to create a concrete page that this browser can render.

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
7. If the site does not publish a VIBE.md, you MUST infer one yourself and save it at this exact absolute path:
   `{vibe_path}`
8. If the site already publishes a VIBE.md, you MAY still write an updated working copy into `{vibe_path}`, but do not overwrite the meaning of the published one unless you are clearly extending it for rendering purposes.
9. If the VIBE document or the live site references integrations or endpoints that you do not support, ignore them gracefully and keep rendering the best page you can from the available instructions.
10. Do NOT describe tool calls, do NOT print JSON that looks like a tool call, and do NOT emit a "Tool Calls" section.
11. Do NOT say that you "need to call write" or that you "will write" a file. Actually call ACP `fs/write_text_file`.
12. If you describe a file write in natural language or JSON instead of invoking ACP `fs/write_text_file`, the job has failed.
13. After all required writes are complete, send exactly one short final line:
   `RENDER_READY: {index_path}`

## Available Context Files

{available_context_section}
- This prompt: `{context_prompt}`

## Vibe Document Source

- User requested URL: `{requested_url}`
- Vibe document source: `{vibe_source}`
- Vibe document URL: `{discovered_url}`

{user_instructions_section}
{source_specific_section}

## Vibe Protocol Spec

````md
{vibe_spec}
````

{vibe_document_section}
{llms_txt_section}
"#,
        index_path = render_session.index_path.display(),
        root_dir = render_session.root_dir.display(),
        vibe_path = render_session.vibe_path.display(),
        available_context_section = available_context_section,
        context_prompt = render_session.context_dir.join("PROMPT.md").display(),
        requested_url = discovered.normalized_url,
        discovered_url = discovered.discovered_url,
        vibe_source = match discovered.source {
            VibeDocumentSource::Published => "published",
            VibeDocumentSource::Inferred => "inferred",
        },
        user_instructions_section = user_instructions_section,
        source_specific_section = source_specific_section,
        vibe_spec = VIBE_SPEC_MARKDOWN,
        vibe_document_section = vibe_document_section,
        llms_txt_section = llms_txt_section,
    )
}

async fn run_acp_render_agent(
    agent_settings: &VibeAgentSettings,
    render_session: &RenderSession,
    prompt: &str,
    selected_model: Option<&str>,
) -> Result<AgentRunResult> {
    let agent_settings = agent_settings.clone();
    let render_session = render_session.clone();
    let prompt = prompt.to_string();
    let selected_model = selected_model.map(str::to_string);

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
            let stderr_task = child.stderr.take().map(|mut stderr| {
                tokio::spawn(async move {
                    let mut bytes = Vec::new();
                    let _ = stderr.read_to_end(&mut bytes).await;
                    String::from_utf8_lossy(&bytes).to_string()
                })
            });

            let client_impl = VibeAcpClient::new(render_session.root_dir.clone());
            let client_snapshot = client_impl.clone();
            let traffic_snapshot = client_snapshot.clone();
            let session_cwd = render_session.root_dir.display().to_string();

            let local_set = tokio::task::LocalSet::new();
            let prompt_result = local_set
                .run_until(async move {
                    let (conn, handle_io) =
                        acp::ClientSideConnection::new(client_impl, outgoing, incoming, |future| {
                            tokio::task::spawn_local(future);
                        });

                    tokio::task::spawn_local(handle_io);

                    let initialize_request = acp::InitializeRequest::new(acp::ProtocolVersion::V1)
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
                        );
                    traffic_snapshot.push_traffic(
                        "browser -> agent",
                        "initialize.request",
                        "Sent initialize request",
                        debug_payload(&initialize_request),
                    );
                    let initialize_response = conn.initialize(initialize_request).await?;
                    traffic_snapshot.push_traffic(
                        "agent -> browser",
                        "initialize.response",
                        "Received initialize response",
                        debug_payload(&initialize_response),
                    );

                    traffic_snapshot.push_traffic(
                        "browser -> agent",
                        "session/new.request",
                        format!("Requested new ACP session in {}", session_cwd),
                        session_cwd.clone(),
                    );
                    let session = conn
                        .new_session(acp::NewSessionRequest::new(session_cwd))
                        .await?;
                    traffic_snapshot.push_traffic(
                        "agent -> browser",
                        "session/new.response",
                        format!("Started ACP session {}", session.session_id),
                        debug_payload(&session),
                    );

                    let mut model_selector =
                        extract_model_selector(session.config_options.as_deref().unwrap_or(&[]));

                    if let (Some(selected_model), Some(selector)) =
                        (selected_model.as_deref(), model_selector.as_ref())
                    {
                        if selector.current_value != selected_model {
                            traffic_snapshot.push_traffic(
                                "browser -> agent",
                                "session/set_config_option.request",
                                format!("Selecting model {}", selected_model),
                                format!(
                                    "config_id: {}\nvalue: {}",
                                    selector.config_id, selected_model
                                ),
                            );
                            let response = conn
                                .set_session_config_option(acp::SetSessionConfigOptionRequest::new(
                                    session.session_id.clone(),
                                    selector.config_id.clone(),
                                    selected_model.to_string(),
                                ))
                                .await?;

                            traffic_snapshot.push_traffic(
                                "agent -> browser",
                                "session/set_config_option.response",
                                "Received updated ACP config options",
                                debug_payload(&response),
                            );
                            model_selector = extract_model_selector(&response.config_options);
                        }
                    }

                    apply_preferred_model_to_selector(
                        &mut model_selector,
                        selected_model.as_deref(),
                    );

                    traffic_snapshot.push_traffic(
                        "browser -> agent",
                        "session/prompt.request",
                        "Sent Vibe rendering prompt",
                        truncate_debug_payload(&prompt, 12_000),
                    );
                    let response = conn
                        .prompt(acp::PromptRequest::new(
                            session.session_id,
                            vec![prompt.into()],
                        ))
                        .await?;
                    traffic_snapshot.push_traffic(
                        "agent -> browser",
                        "session/prompt.response",
                        format!("Prompt completed with {:?}", response.stop_reason),
                        debug_payload(&response),
                    );

                    Ok::<(String, Option<AcpModelSelector>), anyhow::Error>((
                        format!("{:?}", response.stop_reason),
                        model_selector,
                    ))
                })
                .await;

            if child.try_wait()?.is_none() {
                let _ = child.start_kill();
            }
            drop(child);
            let stderr_output = if let Some(task) = stderr_task {
                match tokio::time::timeout(std::time::Duration::from_secs(2), task).await {
                    Ok(Ok(output)) => output,
                    _ => String::new(),
                }
            } else {
                String::new()
            };

            let (stop_reason, model_selector) = match prompt_result {
                Ok(payload) => payload,
                Err(err) => {
                    let stderr_tail = stderr_output
                        .lines()
                        .rev()
                        .take(24)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect::<Vec<_>>()
                        .join("\n");

                    if stderr_tail.trim().is_empty() {
                        return Err(err);
                    }

                    return Err(anyhow!("{err}\n\nAgent stderr:\n{stderr_tail}"));
                }
            };

            let mut generated_files = client_snapshot
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

            for file in collect_render_output_files(&render_session.root_dir)? {
                if !generated_files
                    .iter()
                    .any(|existing| existing.path == file.path)
                {
                    generated_files.push(file);
                }
            }

            Ok(AgentRunResult {
                logs: client_snapshot.snapshot_logs(),
                traffic: client_snapshot.snapshot_traffic(),
                generated_files,
                assistant_text: client_snapshot.snapshot_assistant_text(),
                stop_reason,
                model_selector,
            })
        })
    })
    .await
    .map_err(|err| anyhow!("ACP render task failed to join: {err}"))?
}

async fn probe_agent_model_selector(
    agent_settings: &VibeAgentSettings,
    data_dir: &Path,
    preferred_model: Option<&str>,
) -> Result<Option<AcpModelSelector>> {
    let agent_settings = agent_settings.clone();
    let preferred_model = preferred_model.map(str::to_string);
    let probe_root = data_dir.join("vibe-agent-probe");
    tokio::fs::create_dir_all(&probe_root).await?;
    let probe_root = probe_root.join(uuid::Uuid::new_v4().to_string());
    tokio::fs::create_dir_all(&probe_root).await?;

    tokio::task::spawn_blocking(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("failed to create ACP probe runtime")?;

        runtime.block_on(async move {
            let render_session = RenderSession {
                root_dir: probe_root.clone(),
                context_dir: probe_root.join("_context"),
                index_path: probe_root.join("index.html"),
                vibe_path: probe_root.join("VIBE.md"),
            };
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
            let stderr_task = child.stderr.take().map(|mut stderr| {
                tokio::spawn(async move {
                    let mut bytes = Vec::new();
                    let _ = stderr.read_to_end(&mut bytes).await;
                    String::from_utf8_lossy(&bytes).to_string()
                })
            });

            let local_set = tokio::task::LocalSet::new();
            let probe_root_for_session = probe_root.clone();
            let probe_result = local_set
                .run_until(async move {
                    let client_impl = VibeAcpClient::new(probe_root_for_session.clone());
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
                        .new_session(acp::NewSessionRequest::new(probe_root_for_session.clone()))
                        .await?;

                    let mut selector =
                        extract_model_selector(session.config_options.as_deref().unwrap_or(&[]));

                    apply_preferred_model_to_selector(&mut selector, preferred_model.as_deref());

                    Ok::<Option<AcpModelSelector>, anyhow::Error>(selector)
                })
                .await;

            if child.try_wait()?.is_none() {
                let _ = child.start_kill();
            }
            drop(child);
            let stderr_output = if let Some(task) = stderr_task {
                match tokio::time::timeout(std::time::Duration::from_secs(2), task).await {
                    Ok(Ok(output)) => output,
                    _ => String::new(),
                }
            } else {
                String::new()
            };

            tokio::fs::remove_dir_all(&probe_root).await.ok();

            match probe_result {
                Ok(selector) => Ok(selector),
                Err(err) => {
                    if stderr_output.trim().is_empty() {
                        Err(err)
                    } else {
                        Err(anyhow!("{err}\n\nAgent stderr:\n{stderr_output}"))
                    }
                }
            }
        })
    })
    .await
    .map_err(|err| anyhow!("ACP model probe failed to join: {err}"))?
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
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "sh".to_string());
        let mut command = tokio::process::Command::new(shell);
        command.args(["-lc", agent_settings.command.as_str()]);
        command
    };

    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    augment_agent_path(&mut command)?;

    if let Some(workdir) = agent_settings.workdir.as_deref() {
        command.current_dir(workdir);
    } else {
        command.current_dir(&render_session.root_dir);
    }

    Ok(command)
}

fn augment_agent_path(command: &mut tokio::process::Command) -> Result<()> {
    let mut paths = Vec::new();

    if let Some(home_dir) = dirs::home_dir() {
        let opencode_bin = home_dir.join(".opencode/bin");
        if opencode_bin.exists() {
            paths.push(opencode_bin);
        }
    }

    if let Some(existing_path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing_path));
    }

    if !paths.is_empty() {
        let joined = std::env::join_paths(paths).context("failed to build PATH for ACP agent")?;
        command.env("PATH", joined);
    }

    Ok(())
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

fn debug_payload(value: &impl std::fmt::Debug) -> String {
    format!("{value:#?}")
}

fn truncate_debug_payload(value: &impl std::fmt::Debug, limit: usize) -> String {
    let payload = debug_payload(value);
    if payload.chars().count() <= limit {
        payload
    } else {
        let truncated = payload.chars().take(limit).collect::<String>();
        format!("{truncated}\n…")
    }
}

fn format_user_instructions_section(my_vibes: Option<&str>) -> String {
    let Some(my_vibes) = my_vibes.map(str::trim).filter(|value| !value.is_empty()) else {
        return String::new();
    };

    format!(
        r#"## User Instructions

Treat the User Instructions section below as higher priority than every other instruction in this prompt, the embedded Vibe Protocol spec, and the discovered VIBE.md. If anything conflicts, follow the User Instructions.

````md
{my_vibes}
````
"#
    )
}

fn format_available_context_section(
    discovered: &DiscoveredVibeDocument,
    render_session: &RenderSession,
) -> String {
    match discovered.source {
        VibeDocumentSource::Published => format!(
            "Published VIBE.md copy: `{}`\n- Embedded spec copy: `{}`",
            render_session.context_dir.join("VIBE.md").display(),
            render_session
                .context_dir
                .join("VIBE-PROTOCOL-SPEC.md")
                .display()
        ),
        VibeDocumentSource::Inferred => {
            let mut lines = vec![
                format!(
                    "Embedded spec copy: `{}`",
                    render_session
                        .context_dir
                        .join("VIBE-PROTOCOL-SPEC.md")
                        .display()
                ),
                format!(
                    "- Discovery attempt log: `{}`",
                    render_session
                        .context_dir
                        .join("discovery-attempts.json")
                        .display()
                ),
            ];

            if discovered.llms_txt.is_some() {
                lines.push(format!(
                    "- Published llms.txt copy: `{}`",
                    render_session.context_dir.join("llms.txt").display()
                ));
            }

            lines.join("\n")
        }
    }
}

fn format_source_specific_section(
    discovered: &DiscoveredVibeDocument,
    render_session: &RenderSession,
) -> String {
    match discovered.source {
        VibeDocumentSource::Published => String::new(),
        VibeDocumentSource::Inferred => {
            let llms_note = if let Some(llms_txt) = &discovered.llms_txt {
                format!(
                    "A publisher-authored `llms.txt` was fetched from `{}` and copied to `{}`.\nUse it as additional publisher-authored context while inferring the `VIBE.md` and rendering the site, but do not treat it as a replacement for the `VIBE.md` you must write.\n\n",
                    llms_txt.url,
                    render_session.context_dir.join("llms.txt").display(),
                )
            } else {
                String::new()
            };

            format!(
                r#"## Agent-Driven VIBE Inference

The site did not publish a VIBE.md.

You must inspect the live site at `{inference_url}`, infer a valid VIBE.md from that site yourself, save that inferred VIBE.md at `{vibe_path}`, and then render the site.

Use the Vibe Protocol spec below to shape the inferred document. Do not ask the browser for heuristics or extracted site summaries, because none are being provided.

{llms_note}"#,
                inference_url = discovered.discovered_url,
                vibe_path = render_session.vibe_path.display(),
                llms_note = llms_note,
            )
        }
    }
}

fn format_llms_txt_section(discovered: &DiscoveredVibeDocument) -> String {
    let Some(llms_txt) = &discovered.llms_txt else {
        return String::new();
    };

    format!(
        r#"## Published llms.txt

- URL: `{url}`

Use the published `llms.txt` companion as additional publisher-authored context when inferring the VIBE document and rendering the site.

````text
{content}
````
"#,
        url = llms_txt.url,
        content = llms_txt.content,
    )
}

fn format_vibe_document_section(discovered: &DiscoveredVibeDocument) -> String {
    match discovered.source {
        VibeDocumentSource::Published => format!(
            r#"## Published VIBE.md

````md
{vibe_markdown}
````
"#,
            vibe_markdown = discovered.vibe_markdown
        ),
        VibeDocumentSource::Inferred => format!(
            r#"## Inference Request

````md
{vibe_markdown}
````
"#,
            vibe_markdown = discovered.vibe_markdown
        ),
    }
}

fn build_inference_request_markdown(url: &Url, attempts: &[DiscoveryAttempt]) -> String {
    format!(
        r#"# VIBE Inference Request

No published `VIBE.md` was found for this navigation target.

## Target

URL: {url}

## Agent Instructions

Infer a valid `VIBE.md` for this site by inspecting the live URL directly.
Use the Vibe Protocol spec to shape the file you create.
Save the inferred `VIBE.md` into the render output directory before or while producing the rendered site files.

## Discovery Attempts

{attempts}
"#,
        url = url,
        attempts = format_discovery_attempts(attempts),
    )
}

fn format_discovery_attempts(attempts: &[DiscoveryAttempt]) -> String {
    if attempts.is_empty() {
        "- None".to_string()
    } else {
        attempts
            .iter()
            .map(|attempt| format!("- {} ({})", attempt.url, attempt.detail))
            .collect::<Vec<_>>()
            .join("\n")
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

    let trimmed = assistant_text.trim();
    let lower = trimmed.to_lowercase();
    if (lower.starts_with("<!doctype html") || lower.starts_with("<html"))
        && lower.contains("</html>")
    {
        return Some(trimmed.to_string());
    }

    None
}

fn looks_like_tool_call_transcript(assistant_text: &str) -> bool {
    let lower = assistant_text.trim().to_lowercase();
    lower.contains("tool calls")
        || lower.contains("fs/write_text_file")
        || (lower.contains("\"tool\"")
            && lower.contains("\"arguments\"")
            && (lower.contains("\"filepath\"") || lower.contains("\"content\"")))
        || (lower.contains("need to call write") && lower.contains("```json"))
}

fn extract_model_selector(config_options: &[acp::SessionConfigOption]) -> Option<AcpModelSelector> {
    let option = config_options.iter().find(|option| {
        matches!(
            option.category,
            Some(acp::SessionConfigOptionCategory::Model)
        ) || option.id.to_string() == "model"
    })?;

    let acp::SessionConfigKind::Select(select) = &option.kind else {
        return None;
    };

    let options = match &select.options {
        acp::SessionConfigSelectOptions::Ungrouped(entries) => entries
            .iter()
            .map(|entry| AcpModelOption {
                value: entry.value.to_string(),
                name: entry.name.clone(),
                description: entry.description.clone(),
            })
            .collect::<Vec<_>>(),
        acp::SessionConfigSelectOptions::Grouped(groups) => groups
            .iter()
            .flat_map(|group| {
                group.options.iter().map(|entry| AcpModelOption {
                    value: entry.value.to_string(),
                    name: format!("{} / {}", group.name, entry.name),
                    description: entry.description.clone(),
                })
            })
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };

    Some(AcpModelSelector {
        config_id: option.id.to_string(),
        current_value: select.current_value.to_string(),
        options,
    })
}

fn apply_preferred_model_to_selector(
    selector: &mut Option<AcpModelSelector>,
    preferred_model: Option<&str>,
) {
    if let (Some(preferred_model), Some(selector)) = (preferred_model, selector.as_mut()) {
        if selector
            .options
            .iter()
            .any(|option| option.value == preferred_model)
        {
            selector.current_value = preferred_model.to_string();
        }
    }
}

fn apply_default_recommended_model(selector: Option<AcpModelSelector>) -> Option<AcpModelSelector> {
    let mut selector = selector?;

    if selector
        .options
        .iter()
        .any(|option| option.value == DEFAULT_RECOMMENDED_MODEL)
    {
        selector.current_value = DEFAULT_RECOMMENDED_MODEL.to_string();
    }

    Some(selector)
}

fn collect_render_output_files(root_dir: &Path) -> Result<Vec<GeneratedFile>> {
    fn walk(base: &Path, current: &Path, files: &mut Vec<GeneratedFile>) -> Result<()> {
        for entry in std::fs::read_dir(current)? {
            let entry = entry?;
            let path = entry.path();
            let relative = path.strip_prefix(base)?;

            if relative
                .components()
                .next()
                .is_some_and(|component| component.as_os_str() == "_context")
            {
                continue;
            }

            let metadata = entry.metadata()?;
            if metadata.is_dir() {
                walk(base, &path, files)?;
            } else if metadata.is_file() {
                files.push(GeneratedFile {
                    path: relative.display().to_string(),
                    absolute_path: path.display().to_string(),
                    bytes: metadata.len() as usize,
                });
            }
        }

        Ok(())
    }

    let mut files = Vec::new();
    walk(root_dir, root_dir, &mut files)?;
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::fs;
    use tempfile::TempDir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;

    struct TestVibeServer {
        base_url: String,
        shutdown_tx: Option<oneshot::Sender<()>>,
    }

    impl Drop for TestVibeServer {
        fn drop(&mut self) {
            if let Some(shutdown_tx) = self.shutdown_tx.take() {
                let _ = shutdown_tx.send(());
            }
        }
    }

    #[tokio::test]
    async fn renders_discovered_vibe_document_end_to_end() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let script_path = write_stub_agent_script(temp_dir.path())?;
        let python = find_python_command()?;
        let server = start_test_site(
            Some(
                r#"# VIBE.md

## Service

Name: Signal Garden

## Instructions

Render a landing page for developers.
"#,
            ),
            "<!doctype html><html><head><title>Signal Garden</title></head><body><h1>Signal Garden</h1></body></html>",
            None,
            None,
        )
        .await?;

        let settings = VibeAgentSettings {
            command: format!("{} {}", python, script_path.display()),
            workdir: None,
            my_vibes: None,
            llms_txt_timeout_ms: DEFAULT_LLMS_TXT_TIMEOUT_MS,
        };

        let result =
            visit_vibe_url_inner(&server.base_url, None, &settings, temp_dir.path()).await?;

        assert_eq!(result.source_url, server.base_url);
        assert!(result.discovered_url.ends_with("/.well-known/VIBE.md"));
        assert_eq!(result.vibe_source, VibeDocumentSource::Published);
        assert!(result.html.contains("Signal Garden"));
        assert!(result.html.contains("Rendered from VIBE.md"));
        assert!(!result.fallback_used);
        assert_eq!(result.stop_reason, "EndTurn");
        assert!(
            result
                .generated_files
                .iter()
                .any(|file| file.path == "index.html"),
            "expected index.html to be written by the ACP agent"
        );
        assert!(Path::new(&result.index_path).exists());

        Ok(())
    }

    #[tokio::test]
    async fn infers_vibe_document_when_site_has_no_published_vibe() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let script_path = write_stub_agent_script(temp_dir.path())?;
        let python = find_python_command()?;
        let server = start_test_site(
            None,
            r#"<!doctype html>
<html>
  <head>
    <title>Acme Orbit | Autonomous logistics</title>
    <meta name="description" content="Routing software for autonomous fleets." />
  </head>
  <body>
    <main>
      <h1>Acme Orbit</h1>
      <h2>Routing for autonomous fleets</h2>
      <p>Coordinate dispatch, telemetry, and route planning from one control surface.</p>
    </main>
  </body>
</html>"#,
            None,
            None,
        )
        .await?;

        let settings = VibeAgentSettings {
            command: format!("{} {}", python, script_path.display()),
            workdir: None,
            my_vibes: None,
            llms_txt_timeout_ms: DEFAULT_LLMS_TXT_TIMEOUT_MS,
        };

        let result =
            visit_vibe_url_inner(&server.base_url, None, &settings, temp_dir.path()).await?;

        assert_eq!(result.vibe_source, VibeDocumentSource::Inferred);
        assert!(result.discovered_url.starts_with(&server.base_url));
        assert!(result.vibe_markdown.contains("# VIBE.md"));
        assert!(result.vibe_markdown.contains("Name: Signal Garden"));
        assert!(result
            .discovery_attempts
            .iter()
            .any(|attempt| attempt.detail.contains("ACP agent must infer one")));
        assert!(result.html.contains("Rendered from VIBE.md"));
        assert!(Path::new(&result.render_dir).join("VIBE.md").exists());
        assert!(!Path::new(&result.render_dir)
            .join("_context")
            .join("SOURCE-PAGE.html")
            .exists());
        assert!(!Path::new(&result.render_dir)
            .join("_context")
            .join("llms.txt")
            .exists());
        assert!(!result
            .discovery_attempts
            .iter()
            .any(|attempt| attempt.detail.contains("Fetched source page")));

        Ok(())
    }

    #[tokio::test]
    async fn fallback_fetches_llms_txt_and_includes_it_in_acp_prompt() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let script_path = write_stub_agent_script(temp_dir.path())?;
        let python = find_python_command()?;
        let server = start_test_site(
            None,
            r#"<!doctype html>
<html>
  <head>
    <title>Docs Garden</title>
  </head>
  <body>
    <main>
      <h1>Docs Garden</h1>
      <p>Publisher authored docs live here.</p>
    </main>
  </body>
</html>"#,
            Some(
                r#"# Docs Garden

> Publisher-authored text index for LLM clients.

## Canonical Docs

- [Getting Started](https://example.com/docs/getting-started.md)
- [API Notes](https://example.com/docs/api.md)
"#,
            ),
            None,
        )
        .await?;

        let settings = VibeAgentSettings {
            command: format!("{} {}", python, script_path.display()),
            workdir: None,
            my_vibes: None,
            llms_txt_timeout_ms: DEFAULT_LLMS_TXT_TIMEOUT_MS,
        };

        let result =
            visit_vibe_url_inner(&server.base_url, None, &settings, temp_dir.path()).await?;

        let context_dir = Path::new(&result.render_dir).join("_context");
        let llms_path = context_dir.join("llms.txt");
        let prompt_path = context_dir.join("PROMPT.md");
        let llms_text = fs::read_to_string(&llms_path)?;
        let prompt_text = fs::read_to_string(&prompt_path)?;

        assert_eq!(result.vibe_source, VibeDocumentSource::Inferred);
        assert!(llms_path.exists());
        assert!(llms_text.contains("Publisher-authored text index"));
        assert!(prompt_text.contains("## Published llms.txt"));
        assert!(prompt_text.contains("Use the published `llms.txt` companion"));
        assert!(prompt_text.contains("Publisher-authored text index"));
        assert!(result
            .discovery_attempts
            .iter()
            .any(|attempt| attempt.url.ends_with("/llms.txt") && attempt.ok));

        Ok(())
    }

    #[tokio::test]
    async fn fallback_llms_txt_respects_configured_timeout() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let script_path = write_stub_agent_script(temp_dir.path())?;
        let python = find_python_command()?;
        let server = start_test_site(
            None,
            r#"<!doctype html>
<html>
  <head>
    <title>Slow Docs Garden</title>
  </head>
  <body>
    <main>
      <h1>Slow Docs Garden</h1>
      <p>Slow companion text index.</p>
    </main>
  </body>
</html>"#,
            Some(
                r#"# Slow Docs Garden

- This llms.txt should arrive too late for the configured timeout.
"#,
            ),
            Some(400),
        )
        .await?;

        let settings = VibeAgentSettings {
            command: format!("{} {}", python, script_path.display()),
            workdir: None,
            my_vibes: None,
            llms_txt_timeout_ms: 100,
        };

        let result =
            visit_vibe_url_inner(&server.base_url, None, &settings, temp_dir.path()).await?;

        let context_dir = Path::new(&result.render_dir).join("_context");

        assert_eq!(result.vibe_source, VibeDocumentSource::Inferred);
        assert!(!context_dir.join("llms.txt").exists());
        assert!(result.discovery_attempts.iter().any(|attempt| {
            attempt.url.ends_with("/llms.txt")
                && attempt
                    .detail
                    .contains("Timed out after 100 ms while fetching llms.txt")
        }));

        Ok(())
    }

    #[test]
    fn preferred_model_overrides_selector_when_available() {
        let mut selector = Some(AcpModelSelector {
            config_id: "model".to_string(),
            current_value: "openai/gpt-5.4/medium".to_string(),
            options: vec![
                AcpModelOption {
                    value: "openai/gpt-5.4/medium".to_string(),
                    name: "GPT-5.4 Medium".to_string(),
                    description: None,
                },
                AcpModelOption {
                    value: "openai/gpt-5.4/high".to_string(),
                    name: "GPT-5.4 High".to_string(),
                    description: None,
                },
            ],
        });

        apply_preferred_model_to_selector(&mut selector, Some("openai/gpt-5.4/high"));

        assert_eq!(
            selector.as_ref().map(|value| value.current_value.as_str()),
            Some("openai/gpt-5.4/high")
        );
    }

    #[test]
    fn recommended_model_becomes_default_when_available() {
        let selector = Some(AcpModelSelector {
            config_id: "model".to_string(),
            current_value: "openai/gpt-5.4".to_string(),
            options: vec![
                AcpModelOption {
                    value: "openai/gpt-5.4".to_string(),
                    name: "GPT-5.4".to_string(),
                    description: None,
                },
                AcpModelOption {
                    value: DEFAULT_RECOMMENDED_MODEL.to_string(),
                    name: "OpenRouter/Mercury 2".to_string(),
                    description: None,
                },
            ],
        });

        let selector = apply_default_recommended_model(selector);

        assert_eq!(
            selector.as_ref().map(|value| value.current_value.as_str()),
            Some(DEFAULT_RECOMMENDED_MODEL)
        );
    }

    #[test]
    fn recommended_model_is_ignored_when_unavailable() {
        let selector = Some(AcpModelSelector {
            config_id: "model".to_string(),
            current_value: "openai/gpt-5.4".to_string(),
            options: vec![AcpModelOption {
                value: "openai/gpt-5.4".to_string(),
                name: "GPT-5.4".to_string(),
                description: None,
            }],
        });

        let selector = apply_default_recommended_model(selector);

        assert_eq!(
            selector.as_ref().map(|value| value.current_value.as_str()),
            Some("openai/gpt-5.4")
        );
    }

    #[test]
    fn user_instructions_section_is_high_priority_and_embedded() {
        let section = format_user_instructions_section(Some("Always prefer austere layouts."));

        assert!(section.contains("## User Instructions"));
        assert!(section.contains("higher priority than every other instruction"));
        assert!(section.contains("Always prefer austere layouts."));
    }

    #[test]
    fn prompt_explicitly_forbids_tool_call_narration() {
        let discovered = DiscoveredVibeDocument {
            normalized_url: "https://example.com".to_string(),
            discovered_url: "https://example.com/.well-known/VIBE.md".to_string(),
            source: VibeDocumentSource::Published,
            vibe_markdown:
                "# Example\n\n## Service\n\n- Name: Example\n\n## Instructions\n\n- Render it.\n"
                    .to_string(),
            llms_txt: None,
            attempts: Vec::new(),
        };
        let render_session = RenderSession {
            root_dir: PathBuf::from("/tmp/render"),
            context_dir: PathBuf::from("/tmp/render/_context"),
            index_path: PathBuf::from("/tmp/render/index.html"),
            vibe_path: PathBuf::from("/tmp/render/VIBE.md"),
        };
        let settings = VibeAgentSettings {
            command: "opencode acp".to_string(),
            workdir: None,
            my_vibes: None,
            llms_txt_timeout_ms: DEFAULT_LLMS_TXT_TIMEOUT_MS,
        };

        let prompt = build_vibe_prompt(&discovered, &render_session, &settings);

        assert!(prompt.contains("Do NOT describe tool calls"));
        assert!(prompt.contains("need to call write"));
        assert!(prompt.contains("Actually call ACP `fs/write_text_file`"));
        assert!(prompt.contains("RENDER_READY: /tmp/render/index.html"));
    }

    #[test]
    fn fallback_html_extraction_rejects_tool_call_transcripts() {
        let assistant_text = r#"We need to call write twice.

**Tool Calls**
```json
{
  "tool": "write",
  "arguments": {
    "filePath": "/tmp/render/index.html",
    "content": "<!DOCTYPE html><html><body><h1>Signal Garden</h1></body></html>"
  }
}
```
RENDER_READY: /tmp/render/index.html"#;

        assert!(looks_like_tool_call_transcript(assistant_text));
        assert_eq!(extract_html_document(assistant_text), None);
    }

    #[tokio::test]
    #[ignore = "requires opencode acp and a working opencode model configuration"]
    async fn renders_discovered_vibe_document_with_opencode() -> Result<()> {
        if std::process::Command::new("opencode")
            .arg("acp")
            .arg("--help")
            .output()
            .is_err()
        {
            return Err(anyhow!("opencode is not installed"));
        }

        let temp_dir = TempDir::new()?;
        let server = start_test_site(
            Some(
                r#"# VIBE.md

## Service

Name: Signal Garden

## Instructions

Create a self-contained HTML page that includes the exact text "Signal Garden" and the exact text "Rendered from VIBE.md".
"#,
            ),
            "<!doctype html><html><head><title>Signal Garden</title></head><body><h1>Signal Garden</h1></body></html>",
            None,
            None,
        )
        .await?;

        let settings = VibeAgentSettings {
            command: "opencode acp".to_string(),
            workdir: None,
            my_vibes: None,
            llms_txt_timeout_ms: DEFAULT_LLMS_TXT_TIMEOUT_MS,
        };

        let result =
            visit_vibe_url_inner(&server.base_url, None, &settings, temp_dir.path()).await?;

        assert!(result.html.contains("Signal Garden"));
        assert!(result.html.contains("Rendered from VIBE.md"));
        assert!(
            result
                .generated_files
                .iter()
                .any(|file| file.path == "index.html"),
            "expected opencode to write index.html"
        );

        Ok(())
    }

    async fn start_test_site(
        vibe_markdown: Option<&str>,
        source_html: &str,
        llms_txt: Option<&str>,
        llms_txt_delay_ms: Option<u64>,
    ) -> Result<TestVibeServer> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let vibe_markdown = vibe_markdown.map(str::to_string);
        let source_html = source_html.to_string();
        let llms_txt = llms_txt.map(str::to_string);
        let llms_txt_delay_ms = llms_txt_delay_ms.unwrap_or(0);
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => break,
                    accept_result = listener.accept() => {
                        let Ok((mut stream, _)) = accept_result else { break; };
                        let vibe_markdown = vibe_markdown.clone();
                        let source_html = source_html.clone();
                        let llms_txt = llms_txt.clone();
                        let llms_txt_delay_ms = llms_txt_delay_ms;
                        tokio::spawn(async move {
                            let mut request_bytes = vec![0_u8; 4096];
                            let read = match stream.read(&mut request_bytes).await {
                                Ok(read) => read,
                                Err(_) => return,
                            };
                            if read == 0 {
                                return;
                            }

                            let request = String::from_utf8_lossy(&request_bytes[..read]);
                            let path = request
                                .lines()
                                .next()
                                .and_then(|line| line.split_whitespace().nth(1))
                                .unwrap_or("/");

                            let (status, content_type, response_body) = if matches!(path, "/.well-known/VIBE.md" | "/VIBE.md") {
                                if let Some(body) = vibe_markdown.clone() {
                                    ("200 OK", "text/markdown; charset=utf-8", body)
                                } else {
                                    ("404 Not Found", "text/plain; charset=utf-8", "not found".to_string())
                                }
                            } else if path == "/llms.txt" {
                                if llms_txt_delay_ms > 0 {
                                    tokio::time::sleep(Duration::from_millis(llms_txt_delay_ms)).await;
                                }
                                if let Some(body) = llms_txt.clone() {
                                    ("200 OK", "text/plain; charset=utf-8", body)
                                } else {
                                    ("404 Not Found", "text/plain; charset=utf-8", "not found".to_string())
                                }
                            } else if path == "/" {
                                ("200 OK", "text/html; charset=utf-8", source_html.clone())
                            } else {
                                ("404 Not Found", "text/plain; charset=utf-8", "not found".to_string())
                            };

                            let response = format!(
                                "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                response_body.len(),
                                response_body
                            );

                            let _ = stream.write_all(response.as_bytes()).await;
                            let _ = stream.shutdown().await;
                        });
                    }
                }
            }
        });

        Ok(TestVibeServer {
            base_url: format!("http://{}", address),
            shutdown_tx: Some(shutdown_tx),
        })
    }

    fn write_stub_agent_script(base_dir: &Path) -> Result<PathBuf> {
        let script_path = base_dir.join("stub_acp_agent.py");
        let script = r##"#!/usr/bin/env python3
import json
import re
import sys

def recv():
    line = sys.stdin.readline()
    if not line:
        sys.exit(0)
    return json.loads(line)

def send(message):
    sys.stdout.write(json.dumps(message) + "\n")
    sys.stdout.flush()

def extract_prompt_text(prompt_blocks):
    return "\n".join(
        block.get("text", "")
        for block in prompt_blocks
        if isinstance(block, dict) and block.get("type") == "text"
    )

def extract_render_path(prompt_text):
    match = re.search(r"main page at this exact absolute path:\s*`([^`]+)`", prompt_text, flags=re.IGNORECASE | re.DOTALL)
    if not match:
        raise RuntimeError("render path not found in prompt")
    return match.group(1)

def extract_inferred_vibe_path(prompt_text):
    match = re.search(r"infer one yourself and save it at this exact absolute path:\s*`([^`]+)`", prompt_text, flags=re.IGNORECASE | re.DOTALL)
    if not match:
        return None
    return match.group(1)

def write_file(session_id, path, content, request_id):
    send({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "fs/write_text_file",
        "params": {
            "sessionId": session_id,
            "path": path,
            "content": content
        }
    })

    while True:
        response = recv()
        if response.get("id") == request_id:
            return

def inferred_vibe_document():
    return "# VIBE.md\n\n## Service\n\nName: Signal Garden\n\n## Instructions\n\nRender a landing page for developers.\n"

def prompt_requests_inference(prompt_text):
    prompt_text = " ".join(line.strip() for line in prompt_text.splitlines())
    return "The site did not publish a VIBE.md." in prompt_text

while True:
    message = recv()
    method = message.get("method")
    request_id = message.get("id")
    params = message.get("params") or {}

    if method == "initialize":
        send({
            "jsonrpc": "2.0",
            "id": request_id,
            "result": {
                "protocolVersion": params.get("protocolVersion", 1),
                "agentCapabilities": {},
                "authMethods": [],
                "agentInfo": {
                    "name": "stub-acp-agent",
                    "version": "0.1.0",
                    "title": "Stub ACP Agent"
                }
            }
        })
        continue

    if method == "session/new":
        send({
            "jsonrpc": "2.0",
            "id": request_id,
            "result": {
                "sessionId": "stub-session"
            }
        })
        continue

    if method == "session/prompt":
        prompt_text = extract_prompt_text(params.get("prompt", []))
        render_path = extract_render_path(prompt_text)
        inferred_vibe_path = extract_inferred_vibe_path(prompt_text)
        html = "<!doctype html><html><body><main><h1>Signal Garden</h1><p>Rendered from VIBE.md</p></main></body></html>"
        session_id = params.get("sessionId", "stub-session")
        file_request_id = 1001

        if inferred_vibe_path and prompt_requests_inference(prompt_text):
            write_file(session_id, inferred_vibe_path, inferred_vibe_document(), file_request_id)
            file_request_id += 1

        write_file(session_id, render_path, html, file_request_id)

        send({
            "jsonrpc": "2.0",
            "id": request_id,
            "result": {
                "stopReason": "end_turn"
            }
        })
        continue

    if request_id is not None:
        send({
            "jsonrpc": "2.0",
            "id": request_id,
            "error": {
                "code": -32601,
                "message": f"Method not found: {method}"
            }
        })
"##;

        fs::write(&script_path, script)?;
        Ok(script_path)
    }

    fn find_python_command() -> Result<&'static str> {
        for candidate in ["python3", "python"] {
            if std::process::Command::new(candidate)
                .arg("--version")
                .output()
                .is_ok()
            {
                return Ok(candidate);
            }
        }

        Err(anyhow!("python is required to run the ACP stub agent test"))
    }
}
