use crate::error::Error;
use crate::types::{Parameters, ToolCall, ToolDefinition, ToolMessage, ToolResult};
use async_trait::async_trait;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

// Re-export the proc macro so users can do `use motif::tool;`
pub use macros::tool;

// --- Tool trait ---

/// Whether a tool is safe to execute concurrently with other tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConcurrencySafety {
    /// Safe to run in parallel with other tools (read-only operations).
    ConcurrentSafe,
    /// Must run sequentially (write operations, stateful tools).
    ConcurrentUnsafe,
}

/// Semantic metadata about a tool. Not sent to the LLM; used by
/// Executors, Hooks, and permission systems.
#[derive(Debug, Clone, Default)]
pub struct ToolMetadata {
    /// True if this tool never modifies state.
    pub read_only: bool,
    /// True if calling this tool twice with identical args is safe (no side-effects).
    pub idempotent: bool,
    /// Optional grouping hint, e.g. "file", "network", "compute".
    pub category: Option<String>,
}

/// A callable tool. Accepts JSON string arguments, returns a string result.
#[async_trait]
pub trait Tool: Send + Sync {
    async fn call(&self, args: String) -> String;

    /// Whether this tool is safe to run concurrently with others.
    /// Default: ConcurrentSafe.
    fn concurrency_safety(&self) -> ConcurrencySafety {
        ConcurrencySafety::ConcurrentSafe
    }

    /// Semantic metadata for external systems (hooks, permissions, executors).
    /// Default: all false, no category.
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata::default()
    }
}

// --- FunctionTool: wraps an async fn ---

type ToolFn = Arc<dyn Fn(String) -> Pin<Box<dyn Future<Output = String> + Send>> + Send + Sync>;

pub struct FunctionTool {
    func: ToolFn,
    concurrency: ConcurrencySafety,
}

impl FunctionTool {
    pub fn new<F, Fut>(f: F) -> Self
    where
        F: Fn(String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = String> + Send + 'static,
    {
        Self {
            func: Arc::new(move |args: String| Box::pin(f(args))),
            concurrency: ConcurrencySafety::ConcurrentSafe,
        }
    }

    pub fn with_concurrency(mut self, c: ConcurrencySafety) -> Self {
        self.concurrency = c;
        self
    }
}

#[async_trait]
impl Tool for FunctionTool {
    async fn call(&self, args: String) -> String {
        (self.func)(args).await
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        self.concurrency
    }
}

// --- ToolExecutor trait ---

#[async_trait]
pub trait ToolExecutor: Send + Sync {
    fn register(&mut self, name: String, tool: Arc<dyn Tool>);
    async fn execute(&self, calls: Vec<ToolCall>) -> Vec<ToolResult>;
    fn has(&self, name: &str) -> bool;
}

// --- Executor ---

/// The default tool executor. Runs concurrent-safe tools in parallel,
/// unsafe tools sequentially, preserving the original call order.
pub struct Executor {
    tools: HashMap<String, Arc<dyn Tool>>,
    parallel: bool,
}

impl Executor {
    pub fn parallel() -> Self {
        Self {
            tools: HashMap::new(),
            parallel: true,
        }
    }
    pub fn sequential() -> Self {
        Self {
            tools: HashMap::new(),
            parallel: false,
        }
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::parallel()
    }
}

#[async_trait]
impl ToolExecutor for Executor {
    fn register(&mut self, name: String, tool: Arc<dyn Tool>) {
        self.tools.insert(name, tool);
    }

    async fn execute(&self, calls: Vec<ToolCall>) -> Vec<ToolResult> {
        if calls.is_empty() {
            return vec![];
        }

        if !self.parallel {
            // Sequential path: simple loop, preserves order
            let mut results = Vec::with_capacity(calls.len());
            for call in calls {
                let start = std::time::SystemTime::now();
                let content = match self.tools.get(&call.function.name) {
                    Some(t) => t.call(call.function.arguments).await,
                    None => Error::ToolNotFound {
                        name: call.function.name.clone(),
                        available: self.tools.keys().cloned().collect(),
                    }
                    .to_string(),
                };
                let elapsed = start.elapsed().unwrap_or_default();
                results.push(ToolResult {
                    tool_message: ToolMessage {
                        tool_call_id: call.id,
                        content,
                    },
                    timestamp: start + elapsed,
                    elapsed,
                });
            }
            return results;
        }

        // Parallel path: partition by concurrency safety, preserve order
        let mut indexed: Vec<_> = calls.into_iter().enumerate().collect();
        indexed.sort_by_key(|(_, call)| {
            self.tools
                .get(&call.function.name)
                .map(|t| {
                    if t.concurrency_safety() == ConcurrencySafety::ConcurrentSafe {
                        0
                    } else {
                        1
                    }
                })
                .unwrap_or(0)
        });

        let mut results: Vec<Option<ToolResult>> = vec![None; indexed.len()];

        let (safe, unsafe_calls): (Vec<_>, Vec<_>) = indexed.into_iter().partition(|(_, call)| {
            self.tools
                .get(&call.function.name)
                .map(|t| t.concurrency_safety() == ConcurrencySafety::ConcurrentSafe)
                .unwrap_or(true)
        });

        if !safe.is_empty() {
            use futures::future::join_all;
            let batch = join_all(safe.into_iter().map(|(idx, call)| {
                let tool = self.tools.get(&call.function.name).cloned();
                async move {
                    let start = std::time::SystemTime::now();
                    let content = match tool {
                        Some(t) => t.call(call.function.arguments).await,
                        None => format!(
                            "Tool '{}' not found. Available: {:?}",
                            call.function.name,
                            self.tools.keys().collect::<Vec<_>>()
                        ),
                    };
                    let elapsed = start.elapsed().unwrap_or_default();
                    (
                        idx,
                        ToolResult {
                            tool_message: ToolMessage {
                                tool_call_id: call.id,
                                content,
                            },
                            timestamp: start + elapsed,
                            elapsed,
                        },
                    )
                }
            }))
            .await;
            for (idx, r) in batch {
                results[idx] = Some(r);
            }
        }

        for (idx, call) in unsafe_calls {
            let start = std::time::SystemTime::now();
            let content = match self.tools.get(&call.function.name) {
                Some(t) => t.call(call.function.arguments).await,
                None => format!(
                    "Tool '{}' not found. Available: {:?}",
                    call.function.name,
                    self.tools.keys().collect::<Vec<_>>()
                ),
            };
            let elapsed = start.elapsed().unwrap_or_default();
            results[idx] = Some(ToolResult {
                tool_message: ToolMessage {
                    tool_call_id: call.id,
                    content,
                },
                timestamp: start + elapsed,
                elapsed,
            });
        }

        results.into_iter().map(|r| r.unwrap()).collect()
    }

    fn has(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }
}

// --- ToolDef: builder for manual tool registration (no proc macro yet) ---

pub struct ToolDef {
    name: String,
    description: String,
    properties: serde_json::Map<String, serde_json::Value>,
    required: Vec<String>,
    concurrency: ConcurrencySafety,
}

impl ToolDef {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            properties: serde_json::Map::new(),
            required: vec![],
            concurrency: ConcurrencySafety::ConcurrentSafe,
        }
    }

    pub fn concurrency(mut self, c: ConcurrencySafety) -> Self {
        self.concurrency = c;
        self
    }

    pub fn param<T: schemars::JsonSchema>(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        let name = name.into();
        // Generate JSON schema from the type parameter
        let schema = schemars::schema_for!(T);
        let mut prop_schema = serde_json::to_value(&schema).unwrap_or_default();
        // Add description
        if let Some(obj) = prop_schema.as_object_mut() {
            obj.insert(
                "description".to_string(),
                serde_json::Value::String(description.into()),
            );
            // Remove top-level $schema/title metadata
            obj.remove("$schema");
            obj.remove("title");
        }
        self.properties.insert(name.clone(), prop_schema);
        self.required.push(name);
        self
    }

    pub fn build_definition(&self) -> ToolDefinition {
        let mut params = serde_json::Map::new();
        params.insert(
            "type".to_string(),
            serde_json::Value::String("object".to_string()),
        );
        params.insert(
            "properties".to_string(),
            serde_json::Value::Object(self.properties.clone()),
        );
        params.insert(
            "required".to_string(),
            serde_json::Value::Array(
                self.required
                    .iter()
                    .map(|r| serde_json::Value::String(r.clone()))
                    .collect(),
            ),
        );

        ToolDefinition::new(
            &self.name,
            &self.description,
            Parameters::new(serde_json::Value::Object(params)),
        )
    }

    pub fn build<F, Fut>(self, handler: F) -> RegisteredTool
    where
        F: Fn(String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = String> + Send + 'static,
    {
        let ft = FunctionTool::new(handler).with_concurrency(self.concurrency);
        RegisteredTool {
            definition: self.build_definition(),
            tool: Arc::new(ft),
        }
    }
}

// --- ToolArgs: generated by #[tool] macro ---

/// Provides tool metadata generated by the `#[tool]` proc macro.
/// Users do not implement this manually.
pub trait ToolArgs: schemars::JsonSchema + for<'a> serde::Deserialize<'a> {
    const TOOL_NAME: &'static str;
    const TOOL_DESCRIPTION: &'static str;

    fn definition() -> ToolDefinition {
        ToolDefinition::new(
            Self::TOOL_NAME,
            Self::TOOL_DESCRIPTION,
            Parameters::from_type::<Self>(),
        )
    }
}

impl Parameters {
    /// Create Parameters from a type implementing schemars::JsonSchema.
    pub fn from_type<T: schemars::JsonSchema>() -> Self {
        let mut generator = schemars::SchemaGenerator::default();
        let schema = generator.root_schema_for::<T>();
        let schema_value = serde_json::to_value(&schema).unwrap_or_default();
        let mut map = match schema_value {
            serde_json::Value::Object(m) => m,
            _ => serde_json::Map::new(),
        };
        map.remove("$schema");
        map.remove("title");
        Parameters::new(serde_json::Value::Object(map))
    }
}

// --- RegisteredTool ---

/// A tool ready for registration: definition for the LLM + executable impl.
pub struct RegisteredTool {
    pub definition: ToolDefinition,
    pub tool: Arc<dyn Tool>,
}

impl RegisteredTool {
    pub fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    pub fn into_parts(self) -> (ToolDefinition, Arc<dyn Tool>) {
        (self.definition, self.tool)
    }
}

