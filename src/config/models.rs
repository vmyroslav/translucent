use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    #[serde(default = "default_as_false")]
    pub auto_generate_sessions: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub type_: String,
    pub path: String,
}

fn default_as_false() -> bool {
    false
}

// Default configuration
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
            },
            storage: StorageConfig {
                type_: "memory".to_string(),
                path: "./recordings".to_string(),
            },
            auto_generate_sessions: false,
        }
    }
}