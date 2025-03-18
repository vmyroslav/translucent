use crate::config::AppConfig;
use crate::http::Server;
use crate::session::SessionManager;
use crate::storage::StorageFactory;
use log::info;
use std::sync::Arc;

// Main simulator struct that orchestrates all components
pub struct ApiSimulator {
    config: AppConfig,
    storage: Arc<dyn crate::storage::Storage>,
    session_manager: Arc<SessionManager>,
    server: Server,
}

impl ApiSimulator {
    // Create a new simulator instance
    pub async fn new(config: AppConfig) -> Result<Self, Box<dyn std::error::Error>> {
        info!("Initializing API Simulator");

        // Initialize storage based on configuration
        let storage = StorageFactory::create_storage(&config.storage)?;

        // Initialize session manager with worker threads
        let session_manager = Arc::new(SessionManager::new(storage.clone()));

        // Initialize HTTP server
        let server = Server::new(
            config.server.host.clone(),
            config.server.port,
            session_manager.clone(),
        );

        Ok(Self {
            config,
            storage,
            session_manager,
            server,
        })
    }

    // Run the simulator
    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting API Simulator on {}:{}",
              self.config.server.host, self.config.server.port);

        // Start the HTTP server
        self.server.run().await?;

        Ok(())
    }
}