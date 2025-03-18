mod models;

pub use models::*;
use clap::ArgMatches;
use std::fs;
use std::path::Path;

// Load configuration from file and/or command line arguments
pub fn load_config(matches: ArgMatches) -> Result<AppConfig, Box<dyn std::error::Error>> {
    // Start with default config
    let mut config = AppConfig::default();

    // Load from file if specified
    if let Some(config_path) = matches.get_one::<String>("config") {
        config = load_from_file(config_path)?;
    }

    // Override with command line arguments
    if let Some(port) = matches.get_one::<u16>("port") {
        config.server.port = *port;
    }

    Ok(config)
}

// Load configuration from file
fn load_from_file<P: AsRef<Path>>(path: P) -> Result<AppConfig, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    let config: AppConfig = serde_yaml::from_str(&contents)?;
    Ok(config)
}