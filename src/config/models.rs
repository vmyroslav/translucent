// src/config/models.rs
use serde::{Deserialize, Serialize};
use crate::session::SessionMode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    #[serde(default = "default_as_false")]
    pub auto_generate_sessions: bool,
    #[serde(default)]
    pub proxy: ProxyConfig, // Add the proxy configuration field
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

// New struct for proxy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    #[serde(default)]
    pub default_target: String,
    #[serde(default = "default_as_true")]
    pub forward_host_header: bool,
}

fn default_as_false() -> bool {
    false
}

fn default_as_true() -> bool {
    true
}

fn default_proxy_mode() -> SessionMode {
    SessionMode::Record
}

// Default implementation for AppConfig
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
            proxy: ProxyConfig::default(),
        }
    }
}

// Default implementation for ProxyConfig
impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            default_target: String::new(),
            forward_host_header: true,
        }
    }
}

// Default implementation for ServerConfig
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
        }
    }
}

// Default implementation for StorageConfig
impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            type_: "memory".to_string(),
            path: "./recordings".to_string(),
        }
    }
}