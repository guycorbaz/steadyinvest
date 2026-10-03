//! The rmcp `ServerHandler` of `steadyinvest-mcp` (Story 8.4 AC 2 / 7): tools only — no resource,
//! no prompt. Each call runs on a blocking thread (`McpAccess` is synchronous SQLite with a
//! `busy_timeout` of up to 5 s, which must not stall the stdio transport).

use crate::tools::{self, CallEnv, INSTRUCTIONS};
use chrono::{SecondsFormat, Utc};
use rmcp::model::{
    CacheScope, CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock,
    Implementation, InitializeResult, ListToolsResult, PaginatedRequestParams, ServerCapabilities,
    ServerConfig, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler};
use std::path::PathBuf;
use std::sync::Arc;
use steadyinvest_contract::Timestamp;
use uuid::Uuid;

/// The MCP server: the explicit `--dossier` (if any); everything else is resolved per call.
#[derive(Debug, Clone)]
pub struct SteadyMcp {
    explicit: Option<PathBuf>,
}

impl SteadyMcp {
    pub fn new(explicit: Option<PathBuf>) -> Self {
        SteadyMcp { explicit }
    }
}

/// The `tools/list` result: the eight tools with their runtime-built schemas.
pub fn tools_list() -> Vec<Tool> {
    tools::tool_specs()
        .into_iter()
        .map(|(name, description, schema)| Tool::new(name, description, Arc::new(schema)))
        .collect()
}

fn now() -> Timestamp {
    Timestamp(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true))
}

impl ServerHandler for SteadyMcp {
    fn get_info(&self) -> ServerConfig {
        InitializeResult::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "steadyinvest-mcp",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(INSTRUCTIONS)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        // The cache hints (SEP-2549) are REQUIRED by protocol 2026-07-28 — Claude Code rejects a
        // tools/list without them (Guy's on-screen test, 2026-09-30); rmcp leaves them optional.
        // Private: the server speaks for one owner's dossier; never fresh: nothing is cached.
        Ok(ListToolsResult::with_all_items(tools_list())
            .with_ttl_ms(0)
            .with_cache_scope(CacheScope::Private))
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        tools_list().into_iter().find(|t| t.name == name)
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let name = request.name.to_string();
        if !tools::TOOL_NAMES.contains(&name.as_str()) {
            // Never echo a client-sent name (G3): generic, and nothing logged from it.
            return Err(ErrorData::invalid_params("unknown tool", None));
        }
        if context.ct.is_cancelled() {
            return Err(ErrorData::internal_error("the call was cancelled", None));
        }
        let arguments = request.arguments.unwrap_or_default();
        let explicit = self.explicit.clone();
        let ct = context.ct.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            let cancelled = move || ct.is_cancelled();
            let env = CallEnv {
                explicit: explicit.as_deref(),
                now: &now,
                new_id: &Uuid::new_v4,
                cancelled: &cancelled,
                // FR23: the live verdict ages the price like the app (the owner's horizon).
                price_stale_after: steadyinvest_report::price_age::horizon_or_default(
                    steadyinvest_paths::config_file_path()
                        .and_then(|config| steadyinvest_paths::read_price_stale_after(&config))
                        .as_deref(),
                ),
                // Decision D (Guy 2026-10-03): the trading day is the owner's local day.
                day_zone: steadyinvest_report::price_age::DayZone::Local,
            };
            tools::call(&name, arguments, &env)
        })
        .await
        .map_err(|_| ErrorData::internal_error("tool call aborted", None))?;
        let text = outcome.body.to_string();
        let result = if outcome.is_error {
            CallToolResult::error(vec![ContentBlock::text(text)])
        } else {
            CallToolResult::success(vec![ContentBlock::text(text)])
        };
        Ok(result.into())
    }
}
