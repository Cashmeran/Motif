//! MCP bridge for Motif. Connects Model Context Protocol servers
//! and exposes their tools via [`Agent::external_tools`].
//!
//! ## Usage
//!
//! ```rust,ignore
//! use motif_mcp::McpBridge;
//! use motif::Agent;
//!
//! let bridge = McpBridge::new()
//!     .stdio("weather-server", "weather-mcp-server", &[]);
//!
//! let (defs, handler) = bridge.connect().await.unwrap();
//! let agent = Agent::new(provider).external_tools(defs, handler);
//! ```
//!
//! ## Protocol reference
//!
//! MCP spec: <https://modelcontextprotocol.info/specification/2024-11-05/>
//! - `tools/list` → tool schemas with inputSchema
//! - `tools/call` → call a tool by name with JSON arguments
//! - `notifications/tools/list_changed` → server pushes updates

use motif::RegisteredTool;
use motif::ToolDefinition;
use motif::Parameters;
use std::collections::HashMap;
use std::sync::Arc;

/// Represents a configured MCP server waiting to be connected.
pub enum ServerConfig {
    /// stdio transport: command + args
    Stdio { command: String, args: Vec<String> },
    /// SSE / HTTP transport (URL + optional headers)
    Http { url: String, headers: HashMap<String, String> },
}

/// Bridge that connects MCP servers and produces Motif-compatible tools.
pub struct McpBridge {
    servers: HashMap<String, ServerConfig>,
}

impl McpBridge {
    pub fn new() -> Self { Self { servers: HashMap::new() } }

    /// Add a stdio-based MCP server.
    pub fn stdio(mut self, name: &str, command: &str, args: &[&str]) -> Self {
        self.servers.insert(name.to_string(), ServerConfig::Stdio {
            command: command.to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
        });
        self
    }

    /// Add an HTTP/SSE-based MCP server.
    pub fn http(mut self, name: &str, url: &str) -> Self {
        self.servers.insert(name.to_string(), ServerConfig::Http {
            url: url.to_string(),
            headers: HashMap::new(),
        });
        self
    }

    /// Connect to all servers and return ready-to-register tool definitions
    /// plus a shared handler closure. The handler dispatches tool calls by
    /// server+tool name.
    ///
    /// In a full implementation, this would:
    /// 1. Spawn each server process or open an HTTP connection
    /// 2. Call `initialize` + `tools/list` on each
    /// 3. Convert each tool's JSON Schema to `ToolDefinition`
    /// 4. Return the definitions + a handler that calls `tools/call`
    ///
    /// For now, this is a skeleton that demonstrates the interface contract.
    /// Replace the body with real MCP client logic when you integrate an
    /// MCP SDK (e.g., `mcp-sdk` or `rmcp`).
    pub async fn connect(&self) -> Result<(Vec<ToolDefinition>, impl Fn(String, String) -> String + Send + Sync + 'static), String> {
        let mut defs = Vec::new();

        for (server_name, config) in &self.servers {
            // In production: connect to server, call tools/list
            let tools = match config {
                ServerConfig::Stdio { command, args } => {
                    // TODO: spawn `command` process, initialize MCP, list tools
                    vec![McpToolMeta {
                        name: format!("{}_echo", server_name),
                        description: format!("Echo tool from {} (stdio: {} {})", server_name, command, args.join(" ")),
                    }]
                }
                ServerConfig::Http { url, .. } => {
                    vec![McpToolMeta {
                        name: format!("{}_echo", server_name),
                        description: format!("Echo tool from {} (http: {})", server_name, url),
                    }]
                }
            };

            for tool in &tools {
                defs.push(ToolDefinition::new(
                    &format!("mcp__{}__{}", server_name, tool.name),
                    &tool.description,
                    Parameters::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "args": { "type": "string", "description": "JSON arguments to forward to the MCP tool" }
                        },
                        "required": ["args"]
                    })),
                ));
            }
        }

        let handler = move |_name: String, _args: String| -> String {
            // In production: parse `name` to extract server+tool, call tools/call
            // For now: echo the call
            "MCP call forwarded (replace with real implementation)".to_string()
        };

        Ok((defs, handler))
    }
}

impl Default for McpBridge {
    fn default() -> Self { Self::new() }
}

struct McpToolMeta {
    name: String,
    description: String,
}
