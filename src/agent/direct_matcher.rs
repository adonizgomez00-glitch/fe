use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::agent::direct_executor::{self, DirectMatch, sh_quote};

/// Regla declarativa que mapea un prompt a un comando.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRule {
    pub intent: String,
    pub pattern: String,
    pub command_template: String,
    pub destructive: bool,
}

/// Motor de reglas declarativas.
pub struct DirectMatcher {
    rules: Vec<CommandRule>,
}

impl DirectMatcher {
    pub fn new() -> Self {
        Self {
            rules: vec![],
        }
    }

    pub fn with_file_commands(path: &Path) -> Self {
        let mut matcher = Self::new();
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(extra) = toml::from_str::<Vec<CommandRule>>(&content) {
                    matcher.rules.extend(extra);
                }
            }
        }
        matcher
    }

    pub fn match_prompt(&self, prompt: &str) -> Option<DirectMatch> {
        let prompt = prompt.trim();
        if prompt.is_empty() {
            return None;
        }

        for rule in &self.rules {
            let regex = match regex::Regex::new(&rule.pattern) {
                Ok(r) => r,
                Err(_) => continue,
            };
            if let Some(caps) = regex.captures(prompt) {
                return Some(build_direct_match(rule, &caps));
            }
        }
        None
    }
}

fn build_direct_match(rule: &CommandRule, caps: &regex::Captures) -> DirectMatch {
    let mut command = rule.command_template.clone();
    let mut result = command;

    // Sustituir parámetros {0}, {1}, etc. con capturas
    for i in 0..caps.len() {
        if let Some(cap) = caps.get(i) {
            let placeholder = format!("{{{}}}", i);
            let value = sh_quote(cap.as_str());
            result = result.replace(&placeholder, &value);
        }
    }

    let mut desc = rule.intent.clone();
    desc[0..1].make_ascii_uppercase();

    DirectMatch {
        description: desc,
        command: result,
        is_destructive: rule.destructive,
    }
}

impl Default for DirectMatcher {
    fn default() -> Self {
        Self::new()
    }
}
