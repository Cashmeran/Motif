/// A system prompt composed of a base layer and optional extensions.
/// Extensions are appended in order; each adds its own context block.
#[derive(Clone, Debug)]
pub struct SystemPrompt {
    base: String,
    extensions: Vec<String>,
}

impl SystemPrompt {
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            base: base.into(),
            extensions: vec![],
        }
    }

    /// Append an extension block. Called by external builders (skills,
    /// memory, project context injectors).
    pub fn extend(mut self, block: impl Into<String>) -> Self {
        self.extensions.push(block.into());
        self
    }

    /// Build the final system prompt string.
    pub fn build(&self) -> String {
        if self.extensions.is_empty() {
            return self.base.clone();
        }
        let mut parts = vec![self.base.as_str()];
        for ext in &self.extensions {
            parts.push(ext.as_str());
        }
        parts.join("\n\n---\n\n")
    }
}

impl From<String> for SystemPrompt {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for SystemPrompt {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

/// Trait for external components that want to inject prompt blocks.
/// Implementors are called in registration order before each run.
pub trait PromptBuilder: Send + Sync {
    /// Return a prompt block to append, or None to skip.
    fn build(&self) -> Option<String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_prompt_base_only() {
        let p = SystemPrompt::new("You are a helpful assistant.");
        assert_eq!(p.build(), "You are a helpful assistant.");
    }

    #[test]
    fn test_system_prompt_with_extensions() {
        let p = SystemPrompt::new("Base prompt.")
            .extend("Extension A")
            .extend("Extension B");
        let result = p.build();
        assert!(result.contains("Base prompt."));
        assert!(result.contains("Extension A"));
        assert!(result.contains("Extension B"));
        assert!(result.contains("\n\n---\n\n"));
    }

    #[test]
    fn test_prompt_builder_trait_is_object_safe() {
        // Compile-time check: PromptBuilder can be used as trait object
        struct TestBuilder;
        impl PromptBuilder for TestBuilder {
            fn build(&self) -> Option<String> {
                Some("test".into())
            }
        }
        let builder: Box<dyn PromptBuilder> = Box::new(TestBuilder);
        assert_eq!(builder.build(), Some("test".to_string()));
    }
}
