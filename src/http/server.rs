use crate::session::SessionManager;
use axum::{
    Router,
    routing::{get, post, delete, any},
};
use log::info;
use std::net::SocketAddr;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;

use super::handlers::{
    get_server_info,
    list_sessions,
    create_session,
    delete_session,
    handle_api_request,
};

// HTTP server that handles API simulator requests
pub struct Server {
    host: String,
    port: u16,
    session_manager: Arc<SessionManager>,
}

impl Server {
    // Create a new server
    pub fn new(host: String, port: u16, session_manager: Arc<SessionManager>) -> Self {
        Self {
            host,
            port,
            session_manager,
        }
    }

    // Run the server
    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        // Setup app state with session manager
        let state = crate::http::handlers::AppState {
            session_manager: self.session_manager.clone(),
        };

        // Create the router with all routes
        let app = Router::new()
            // Control API routes
            .route("/__api_simulator/info", get(get_server_info))
            .route("/__api_simulator/sessions", get(list_sessions).post(create_session))
            .route("/__api_simulator/sessions/:id", delete(delete_session))
            // Main API simulator route - handle all other requests
            .fallback(handle_api_request)
            .with_state(state)
            // Add middleware
            .layer(
                ServiceBuilder::new()
                    .layer(TraceLayer::new_for_http())
            );

        // Parse the socket address
        let addr: SocketAddr = format!("{}:{}", self.host, self.port).parse()?;

        // Start the server
        info!("Server started on http://{}", addr);
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}