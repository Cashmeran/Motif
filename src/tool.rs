use async_trait::async_trait;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use crate::types::{Parameters, ToolCall, ToolDefinition, ToolMessage, ToolResult};

// --- Tool trait ---

/// A callable tool. Accepts JSON string arguments, returns a string result.
#[async_trait]
pub trait Tool: Send + Sync {
    async fn call(&self, args: String) -> String;
}

// --- FunctionTool: wraps an async fn ---

type ToolFn = Arc<
    dyn Fn(String) -> Pin<Box<dyn Future<Output = String> + Send>>
        + Send
        + Sync,
>;

pub struct FunctionTool {
    func: ToolFn,
}

impl FunctionTool {
    pub fn new<F, Fut>(f: F) -> Self
    where
        F: Fn(String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = String> + Send + 'static,
    {
        Self {
            func: Arc::new(move |args: String| Box::pin(f(args))),
        }
    }
}

#[async_trait]
impl Tool for FunctionTool {
    async fn call(&self, args: String) -> String {
        (self.func)(args).await
    }
}

// --- ToolExecutor trait ---

#[async_trait]
pub trait ToolExecutor: Send + Sync {
    fn register(&mut self, name: String, tool: Arc<dyn Tool>);
    async fn execute(&self, calls: Vec<ToolCall>) -> Vec<ToolResult>;
    fn has(&self, name: &str) -> bool;
}

// --- ParallelExecutor ---

pub struct ParallelExecutor {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ParallelExecutor {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }
}

impl Default for ParallelExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolExecutor for ParallelExecutor {
    fn register(&mut self, name: String, tool: Arc<dyn Tool>) {
        self.tools.insert(name, tool);
    }

    async fn execute(&self, calls: Vec<ToolCall>) -> Vec<ToolResult> {
        let futures: Vec<_> = calls
            .into_iter()
            .map(|call| {
                let tool = self.tools.get(&call.function.name).cloned();
                async move {
                    let start = std::time::SystemTime::now();
                    let content = match tool {
                        Some(t) => t.call(call.function.arguments).await,
                        None => format!("Tool '{}' not found", call.function.name),
                    };
                    let elapsed = start.elapsed().unwrap_or_default();
                    ToolResult {
                        tool_message: ToolMessage {
                            tool_call_id: call.id,
                            content,
                        },
                        timestamp: start + elapsed,
                        elapsed,
                    }
                }
            })
            .collect();

        // Execute in parallel
        use futures::future::join_all;
        join_all(futures).await
    }

    fn has(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }
}

// --- SequentialExecutor ---

pub struct SequentialExecutor {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl SequentialExecutor {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }
}

impl Default for SequentialExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolExecutor for SequentialExecutor {
    fn register(&mut self, name: String, tool: Arc<dyn Tool>) {
        self.tools.insert(name, tool);
    }

    async fn execute(&self, calls: Vec<ToolCall>) -> Vec<ToolResult> {
        let mut results = Vec::with_capacity(calls.len());
        for call in calls {
            let start = std::time::SystemTime::now();
            let content = match self.tools.get(&call.function.name) {
                Some(t) => t.call(call.function.arguments).await,
                None => format!("Tool '{}' not found", call.function.name),
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
        results
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
}

impl ToolDef {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            properties: serde_json::Map::new(),
            required: vec![],
        }
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
            obj.insert("description".to_string(), serde_json::Value::String(description.into()));
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
        params.insert("type".to_string(), serde_json::Value::String("object".to_string()));
        params.insert("properties".to_string(), serde_json::Value::Object(self.properties.clone()));
        params.insert("required".to_string(), serde_json::Value::Array(
            self.required.iter().map(|r| serde_json::Value::String(r.clone())).collect()
        ));

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
        RegisteredTool {
            definition: self.build_definition(),
            tool: Arc::new(FunctionTool::new(handler)),
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_def_builds_definition() {
        let def = ToolDef::new("greet", "Say hello")
            .param::<String>("name", "Who to greet")
            .build_definition();

        assert_eq!(def.function.name, "greet");
        assert!(def.function.description.contains("hello"));
    }

    #[tokio::test]
    async fn test_parallel_executor_executes_tools() {
        let mut exec = ParallelExecutor::new();
        let tool = Arc::new(FunctionTool::new(|args: String| {
            Box::pin(async move { format!("got: {}", args) })
        }));
        exec.register("echo".into(), tool);

        let results = exec
            .execute(vec![ToolCall {
                id: "call_1".into(),
                call_type: "function".into(),
                function: crate::types::FunctionCall {
                    name: "echo".into(),
                    arguments: r#"{"msg":"hi"}"#.into(),
                },
            }])
            .await;

        assert_eq!(results.len(), 1);
        assert!(results[0].tool_message.content.contains("got:"));
    }

    #[tokio::test]
    async fn test_tool_not_found_returns_error_message() {
        let exec = ParallelExecutor::new();
        let results = exec
            .execute(vec![ToolCall {
                id: "call_1".into(),
                call_type: "function".into(),
                function: crate::types::FunctionCall {
                    name: "nonexistent".into(),
                    arguments: "{}".into(),
                },
            }])
            .await;

        assert_eq!(results.len(), 1);
        assert!(results[0].tool_message.content.contains("not found"));
    }

    #[test]
    fn test_registered_tool_into_parts() {
        let registered = ToolDef::new("ping", "Ping the server")
            .build(|_args: String| async { "pong".to_string() });
        let (def, tool) = registered.into_parts();
        assert_eq!(def.function.name, "ping");
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(tool.call("{}".into()));
        assert_eq!(result, "pong");
    }
}
