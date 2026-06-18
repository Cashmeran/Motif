//! Skill system for Motif. Loads SKILL.md files and exposes them
//! as [`PromptBuilder`] implementations.
//!
//! ## Skill file format (SKILL.md)
//!
//! ```markdown
//! ---
//! name: code-review
//! description: Review code for bugs and style issues
//! when_to_use: When the user asks for a code review
//! allowed_tools: [Read, Grep, Glob]
//! ---
//!
//! # Code Review Skill
//!
//! When reviewing code, you should:
//! 1. Read the changed files
//! 2. Check for bugs, style issues, and missing error handling
//! 3. Report findings with file paths and line numbers
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use motif_skill::SkillLoader;
//! use motif::Agent;
//!
//! let loader = SkillLoader::new();
//! let skill = loader.load_from_file(".motif/skills/code-review/SKILL.md").unwrap();
//! let agent = Agent::new(provider)
//!     .prompt_builder(skill);
//! ```
//!
//! Note: a `Skill` wraps the prompt content; tool registration is handled
//! separately by the caller. When `allowed_tools` is specified in the
//! frontmatter, the caller can register the required tools before passing
//! the agent to chat.

use motif::PromptBuilder;
use serde::Deserialize;
use std::path::Path;

/// Parsed YAML frontmatter from a SKILL.md file.
#[derive(Deserialize, Default)]
struct Frontmatter {
    name: Option<String>,
    description: Option<String>,
    when_to_use: Option<String>,
    #[serde(default)]
    allowed_tools: Vec<String>,
    #[serde(default)]
    user_invocable: bool,
    #[serde(default)]
    disable_model_invocation: bool,
}

/// A loaded skill. Implements [`PromptBuilder`] so it can be injected
/// directly into an Agent's system prompt.
pub struct Skill {
    name: String,
    description: String,
    when_to_use: String,
    body: String,
}

impl Skill {
    pub fn name(&self) -> &str { &self.name }
    pub fn description(&self) -> &str { &self.description }
    pub fn when_to_use(&self) -> &str { &self.when_to_use }
    pub fn allowed_tools(&self) -> &[String] { &[] }
}

impl PromptBuilder for Skill {
    fn build(&self) -> Option<String> {
        let header = if self.when_to_use.is_empty() {
            format!("# Skill: {}\n{}", self.name, self.description)
        } else {
            format!("# Skill: {}\n{}\n\nWhen to use: {}", self.name, self.description, self.when_to_use)
        };
        Some(format!("{}\n\n{}", header, self.body))
    }
}

/// Loads skills from the filesystem. Each skill is a directory containing
/// a `SKILL.md` file with optional YAML frontmatter.
pub struct SkillLoader {
    search_paths: Vec<String>,
}

impl SkillLoader {
    /// Create a loader that searches the default paths:
    /// `~/.motif/skills/` and `./.motif/skills/`.
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_default().join(".motif").join("skills");
        Self {
            search_paths: vec![
                home.to_string_lossy().to_string(),
                ".motif/skills".to_string(),
            ],
        }
    }

    /// Add a custom search path.
    pub fn with_path(mut self, path: &str) -> Self {
        self.search_paths.push(path.to_string());
        self
    }

    /// Load a skill from a specific SKILL.md file path.
    pub fn load_from_file(&self, path: &str) -> Result<Skill, String> {
        let p = Path::new(path);
        let content = std::fs::read_to_string(p)
            .map_err(|e| format!("Cannot read {}: {}", path, e))?;
        Self::parse(&content).ok_or_else(|| format!("Failed to parse {}", path))
    }

    /// Discover all skills in the search paths. Each skill is a
    /// directory containing a SKILL.md file.
    pub fn discover(&self) -> Vec<Skill> {
        let mut skills = Vec::new();
        for base in &self.search_paths {
            let base = Path::new(base);
            if !base.exists() { continue; }
            if let Ok(entries) = std::fs::read_dir(base) {
                for entry in entries.flatten() {
                    let skill_dir = entry.path();
                    if !skill_dir.is_dir() { continue; }
                    let skill_md = skill_dir.join("SKILL.md");
                    if skill_md.exists() {
                        if let Ok(s) = self.load_from_file(&skill_md.to_string_lossy()) {
                            skills.push(s);
                        }
                    }
                }
            }
        }
        skills
    }

    /// Parse a SKILL.md string with optional YAML frontmatter (`---` delimiters).
    fn parse(content: &str) -> Option<Skill> {
        let body = content.trim();
        let (frontmatter, markdown_body) = if body.starts_with("---") {
            let rest = &body[3..];
            if let Some(end) = rest.find("---") {
                let fm_str = &rest[..end];
                let md = rest[end + 3..].trim();
                (serde_yaml::from_str::<Frontmatter>(fm_str).unwrap_or_default(), md.to_string())
            } else {
                (Frontmatter::default(), body.to_string())
            }
        } else {
            (Frontmatter::default(), body.to_string())
        };

        let name = frontmatter.name.unwrap_or_else(|| "unnamed".to_string());
        let desc = frontmatter.description.unwrap_or_else(|| "No description".to_string());
        let when = frontmatter.when_to_use.unwrap_or_default();

        Some(Skill { name, description: desc, when_to_use: when, body: markdown_body })
    }
}

impl Default for SkillLoader {
    fn default() -> Self { Self::new() }
}
