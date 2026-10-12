//! The MCP wire layer: a small, fixed set of tools over [`AgentSession`]. Tool names are
//! snake_case; arguments are strict (unknown fields are rejected).

use std::sync::{Arc, Mutex, PoisonError};

use base64::Engine as _;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock as Content};
use rmcp::{ErrorData as McpError, ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::session::{AgentSession, BatchStep};

const INSTRUCTIONS: &str = "\
Resolution is a video editor modelled on DaVinci Resolve. You edit by running commands: the same \
commands its menus and keyboard shortcuts run, in one undo history shared with the user. Command \
ids follow Resolve's names and pages (timeline.split, timeline.rippleDelete, edit.insert, mark.in…).

Workflow:
1. list_pages, then search_commands (by page or text; results include Resolve's shortcut keys).
2. inspect what=timeline to see the current state.
3. checkpoint before multi-step work.
4. run (or run_batch). Use dry_run: true when unsure: it reports what would change on the \
timeline and reverts.
5. render_frame to look at the result.
6. Back out with undo or restore_checkpoint.

Destructive commands (opening or saving project files, deleting files) cannot be undone: confirm \
with the user, then pass acknowledge: true. Commands named raw.<engine>/… reach the underlying \
engine directly; prefer Resolution commands, which stay stable across engine updates.";

const TIMELINE_URI: &str = "resolution://timeline";
const COMMANDS_URI: &str = "resolution://commands";

#[derive(Clone)]
pub struct ResolutionMcp {
    session: Arc<Mutex<AgentSession>>,
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchParams {
    /// Text to match against command id, label, or a Resolve shortcut such as `Mod+B` or `F9`.
    #[serde(default)]
    pub query: Option<String>,
    /// Only this page: global, media, cut, edit, fusion, color, fairlight, deliver.
    #[serde(default)]
    pub page: Option<String>,
    /// Also search the engine's own commands (`raw.<engine>/…`). Default false.
    #[serde(default)]
    pub include_raw: Option<bool>,
    /// Most results to return. Default 40.
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunParams {
    /// Command id, e.g. `timeline.split`, `mark.in`, `playhead.set`.
    pub command: String,
    /// Parameters object, as documented by search_commands. Default {}.
    #[serde(default)]
    pub params: Option<Value>,
    /// Measure the change, then revert it. Default false.
    #[serde(default)]
    pub dry_run: Option<bool>,
    /// Required for destructive commands, after the user has agreed.
    #[serde(default)]
    pub acknowledge: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub command: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BatchParams {
    /// Commands to run in order.
    pub steps: Vec<Step>,
    /// Run everything, report the combined change, then revert it all. Default false.
    #[serde(default)]
    pub dry_run: Option<bool>,
    /// On a failing step, undo the steps that already ran. Default true.
    #[serde(default)]
    pub atomic: Option<bool>,
    /// Required if any step is destructive.
    #[serde(default)]
    pub acknowledge: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InspectParams {
    /// `timeline` (default), `project`, or `session` (agent edits, checkpoints).
    #[serde(default)]
    pub what: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenderParams {
    /// Timeline time in seconds. Default: the playhead (which is never moved).
    #[serde(default)]
    pub seconds: Option<f64>,
    /// Longest side of the PNG in pixels. Default 960.
    #[serde(default)]
    pub max_side: Option<u32>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CountParams {
    /// How many steps. Default 1.
    #[serde(default)]
    pub steps: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NameParams {
    pub name: String,
}

fn ok_json(v: &Value) -> CallToolResult {
    CallToolResult::success(vec![Content::text(serde_json::to_string_pretty(v).unwrap_or_default())])
}

fn reply(r: Result<Value, String>) -> Result<CallToolResult, McpError> {
    Ok(match r {
        Ok(v) => ok_json(&v),
        Err(e) => CallToolResult::error(vec![Content::text(e)]),
    })
}

impl ResolutionMcp {
    pub fn new(session: AgentSession) -> Self {
        Self { session: Arc::new(Mutex::new(session)), tool_router: Self::tool_router() }
    }

    /// Run `f` on the session off the async runtime. A panic inside the engine becomes an error
    /// for this call; the session stays usable.
    async fn with<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut AgentSession) -> Result<T, String> + Send + 'static,
    ) -> Result<T, String> {
        let s = self.session.clone();
        tokio::task::spawn_blocking(move || f(&mut s.lock().unwrap_or_else(PoisonError::into_inner)))
            .await
            .unwrap_or_else(|e| Err(format!("internal error: {e}")))
    }

    /// Serve MCP on stdin/stdout until the client disconnects.
    pub async fn serve_stdio(self) -> Result<(), String> {
        let running = self.serve(rmcp::transport::stdio()).await.map_err(|e| format!("MCP init: {e}"))?;
        running.waiting().await.map(|_| ()).map_err(|e| e.to_string())
    }
}

#[tool_router]
impl ResolutionMcp {
    #[tool(
        title = "List pages",
        description = "Resolve's pages (Media, Cut, Edit, Fusion, Color, Fairlight, Deliver, plus global commands) and how many commands each offers. Start here.",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn list_pages(&self) -> Result<CallToolResult, McpError> {
        reply(self.with(|s| Ok(s.pages())).await)
    }

    #[tool(
        title = "Search commands",
        description = "Find commands by text, page, or Resolve shortcut key. Each result has id, label, page, risk (read/edit/destructive), params, Resolve keys, and any difference from Resolve's behaviour.",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn search_commands(&self, Parameters(p): Parameters<SearchParams>) -> Result<CallToolResult, McpError> {
        reply(
            self.with(move |s| {
                s.search(p.query.as_deref(), p.page.as_deref(), p.include_raw.unwrap_or(false), p.limit.unwrap_or(40))
            })
            .await,
        )
    }

    #[tool(
        title = "Run a command",
        description = "Run one command: {command, params?, dry_run?, acknowledge?}. Returns the result, its undo label, and what changed on the timeline. dry_run measures the change and reverts it. Destructive commands need acknowledge: true.",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = false, open_world_hint = false)
    )]
    async fn run(&self, Parameters(p): Parameters<RunParams>) -> Result<CallToolResult, McpError> {
        reply(
            self.with(move |s| {
                s.run(
                    &p.command,
                    p.params.unwrap_or_else(|| json!({})),
                    p.dry_run.unwrap_or(false),
                    p.acknowledge.unwrap_or(false),
                )
            })
            .await,
        )
    }

    #[tool(
        title = "Run several commands",
        description = "Run commands in order: {steps: [{command, params?}], dry_run?, atomic? (default true), acknowledge?}. Every step is checked before any runs; with atomic, a failure undoes the steps already run. Reports the combined timeline change. Undo and redo cannot be steps: use the undo and redo tools.",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = false, open_world_hint = false)
    )]
    async fn run_batch(&self, Parameters(p): Parameters<BatchParams>) -> Result<CallToolResult, McpError> {
        let steps: Vec<BatchStep> = p
            .steps
            .into_iter()
            .map(|s| BatchStep { command: s.command, params: s.params.unwrap_or_else(|| json!({})) })
            .collect();
        reply(
            self.with(move |s| {
                s.run_batch(steps, p.dry_run.unwrap_or(false), p.acknowledge.unwrap_or(false), p.atomic.unwrap_or(true))
            })
            .await,
        )
    }

    #[tool(
        title = "Inspect",
        description = "Current state as JSON: what = timeline (default; tracks, clips, marks, playhead), project (media pool and timelines), or session (edits made through this connection, checkpoints).",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn inspect(&self, Parameters(p): Parameters<InspectParams>) -> Result<CallToolResult, McpError> {
        reply(self.with(move |s| s.inspect(p.what.as_deref().unwrap_or("timeline"))).await)
    }

    #[tool(
        title = "Render a frame",
        description = "The program viewer as a PNG image, at `seconds` or the playhead. Does not move the playhead. Use it to check edits visually.",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn render_frame(&self, Parameters(p): Parameters<RenderParams>) -> Result<CallToolResult, McpError> {
        let max = p.max_side.unwrap_or(960).clamp(16, 4096);
        let r = self.with(move |s| s.render_png(p.seconds, max)).await;
        Ok(match r {
            Ok(png) => CallToolResult::success(vec![
                Content::image(base64::engine::general_purpose::STANDARD.encode(png), "image/png"),
                Content::text(match p.seconds {
                    Some(t) => format!("program viewer at {t} s"),
                    None => "program viewer at the playhead".into(),
                }),
            ]),
            Err(e) => CallToolResult::error(vec![Content::text(e)]),
        })
    }

    #[tool(
        title = "Undo",
        description = "Undo the last {steps} edits (default 1). The history is shared with the user.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn undo(&self, Parameters(p): Parameters<CountParams>) -> Result<CallToolResult, McpError> {
        reply(self.with(move |s| Ok(s.undo(p.steps.unwrap_or(1)))).await)
    }

    #[tool(
        title = "Redo",
        description = "Redo the last {steps} undone edits (default 1).",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn redo(&self, Parameters(p): Parameters<CountParams>) -> Result<CallToolResult, McpError> {
        reply(self.with(move |s| Ok(s.redo(p.steps.unwrap_or(1)))).await)
    }

    #[tool(
        title = "Checkpoint",
        description = "Remember this point in the edit history under a name, to return to with restore_checkpoint.",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn checkpoint(&self, Parameters(p): Parameters<NameParams>) -> Result<CallToolResult, McpError> {
        reply(self.with(move |s| Ok(s.checkpoint(&p.name))).await)
    }

    #[tool(
        title = "Restore checkpoint",
        description = "Undo every edit made through this connection since the named checkpoint.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn restore_checkpoint(&self, Parameters(p): Parameters<NameParams>) -> Result<CallToolResult, McpError> {
        reply(self.with(move |s| s.restore(&p.name)).await)
    }
}

/// MCP 2026-07-28 clients require cache hints on list and read results.
fn cache_hints(
    context: &rmcp::service::RequestContext<rmcp::RoleServer>,
    ttl_ms: u64,
) -> Option<(u64, rmcp::model::CacheScope)> {
    context
        .protocol_version()
        .is_some_and(|v| v >= rmcp::model::ProtocolVersion::V_2026_07_28)
        .then_some((ttl_ms, rmcp::model::CacheScope::Private))
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for ResolutionMcp {
    fn get_info(&self) -> rmcp::model::ServerConfig {
        rmcp::model::ServerConfig::new(
            rmcp::model::ServerCapabilities::builder().enable_tools().enable_resources().build(),
        )
        .with_server_info(rmcp::model::Implementation::new("resolution", env!("CARGO_PKG_VERSION")))
        .with_instructions(INSTRUCTIONS.to_string())
    }

    async fn list_resources(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ListResourcesResult, McpError> {
        use rmcp::model::Resource;
        let mut r = rmcp::model::ListResourcesResult::with_all_items(vec![
            Resource::new(TIMELINE_URI, "timeline")
                .with_title("Timeline")
                .with_description("The active timeline (same as inspect what=timeline).")
                .with_mime_type("application/json"),
            Resource::new(COMMANDS_URI, "commands")
                .with_title("Command catalog")
                .with_description("Every Resolution command with page, risk, params and Resolve keys.")
                .with_mime_type("application/json"),
        ]);
        if let Some((ttl, scope)) = cache_hints(&context, 600_000) {
            r = r.with_ttl_ms(ttl).with_cache_scope(scope);
        }
        Ok(r)
    }

    async fn read_resource(
        &self,
        request: rmcp::model::ReadResourceRequestParams,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ReadResourceResponse, McpError> {
        let v = match request.uri.as_str() {
            TIMELINE_URI => self.with(|s| s.inspect("timeline")).await,
            COMMANDS_URI => self.with(|s| s.search(None, None, false, usize::MAX)).await,
            other => return Err(McpError::resource_not_found(format!("resource not found: {other}"), None)),
        }
        .map_err(|e| McpError::internal_error(e, None))?;
        let text = serde_json::to_string_pretty(&v).map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let mut r = rmcp::model::ReadResourceResult::new(vec![
            rmcp::model::ResourceContents::text(text, request.uri).with_mime_type("application/json"),
        ]);
        if let Some((ttl, scope)) = cache_hints(&context, 0) {
            r = r.with_ttl_ms(ttl).with_cache_scope(scope);
        }
        Ok(r.into())
    }
}
