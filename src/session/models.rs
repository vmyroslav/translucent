use serde::{Serialize, Deserialize};
use std::collections::HashMap;

pub type SessionId = String;

// Operation modes for a session
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionMode {
    Record,
    Replay,
    Passthrough,
}

// Session configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub mode: SessionMode,
    pub target_url: Option<String>,
    pub dynamic_patterns: Vec<DynamicPattern>,
}

// Dynamic pattern definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicPattern {
    pub pattern: String,
    pub generator: String,
    pub params: HashMap<String, String>,
}

// Default implementation for SessionConfig
impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            mode: SessionMode::Record,
            target_url: None,
            dynamic_patterns: Vec::new(),
        }
    }
}