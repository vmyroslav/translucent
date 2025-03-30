use serde::{Serialize, Deserialize};

pub type SessionId = String;

// Operation modes for a session
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionMode {
    Record,
    Replay,
}

// Session configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub mode: SessionMode,
}

// Default implementation for SessionConfig
impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            mode: SessionMode::Record,
        }
    }
}