//! Resolution's MCP server. Agents get a small, fixed tool set (find commands, run them safely,
//! look at the result, back out) over the same engine commands and undo history people use.
//! Engine-agnostic: it only sees `resolution_engine_api::Engine`.

pub mod server;
pub mod session;

pub use server::ResolutionMcp;
pub use session::AgentSession;

use resolution_engine_api::Engine;

/// Serve MCP over stdio on `engine` until the client disconnects (blocks; owns its runtime).
pub fn serve_stdio(engine: Box<dyn Engine + Send>) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().map_err(|e| e.to_string())?;
    rt.block_on(ResolutionMcp::new(AgentSession::new(engine)).serve_stdio())
}
