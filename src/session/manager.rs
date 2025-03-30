use crate::matching::{RequestMatcher, MatchResult};
use crate::storage::Storage;
use crate::session::{SessionId, SessionConfig, SessionMode};

use axum::{
    body::{Bytes, Body, to_bytes},
    extract::Request,
    response::{Response},
    http::{StatusCode, HeaderMap, Uri, Method},
};

use http_body_util::{BodyExt, Full, Empty};
use hyper::body::Incoming;
use hyper_rustls::{HttpsConnectorBuilder};
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{RwLock, Mutex};
use log::{debug, info, error};

// Session manager that handles multiple sessions
pub struct SessionManager {
    storage: Arc<dyn Storage>,
    sessions: RwLock<HashMap<SessionId, Arc<Session>>>,
    app_config: Option<crate::config::AppConfig>,
}

struct Session {
    id: SessionId,
    config: RwLock<SessionConfig>,
    matcher: Arc<RequestMatcher>,
    storage: Arc<dyn Storage>,
    dynamic_values: RwLock<HashMap<String, String>>,
    last_access: Mutex<Instant>,
}

impl SessionManager {
    // Create a new session manager
    pub fn new(storage: Arc<dyn Storage>, app_config: Option<crate::config::AppConfig>) -> Self {
        Self {
            storage,
            sessions: RwLock::new(HashMap::new()),
            app_config,
        }
    }

    // Get current session count
    pub fn get_session_count(&self) -> usize {
        // This is not real-time accurate but good enough for info purposes
        self.sessions.try_read().map(|s| s.len()).unwrap_or(0)
    }

    // Check if a session exists
    pub async fn session_exists(&self, id: &str) -> bool {
        let sessions = self.sessions.read().await;
        sessions.contains_key(id)
    }

    // Create a new session
    pub async fn create_session(&self, id: SessionId) -> Result<(), String> {
        let mut sessions = self.sessions.write().await;

        if sessions.contains_key(&id) {
            return Err(format!("Session {} already exists", id));
        }

        // Get proxy config defaults from app config if available
        let default_mode = SessionMode::Record;

        let default_target = match &self.app_config {
            Some(config) => config.proxy.default_target.clone(),
            None => String::new(),
        };

        // Create session config with defaults
        let config = SessionConfig {
            mode: default_mode,
        };

        // Create matcher
        let matcher = Arc::new(RequestMatcher::new());

        // Create session
        let session = Arc::new(Session {
            id: id.clone(),
            config: RwLock::new(config),
            matcher,
            storage: self.storage.clone(),
            dynamic_values: RwLock::new(HashMap::new()),
            last_access: Mutex::new(Instant::now()),
        });

        sessions.insert(id, session);

        Ok(())
    }

    // Delete a session
    pub async fn delete_session(&self, id: &str) -> Result<(), String> {
        let mut sessions = self.sessions.write().await;

        if sessions.remove(id).is_none() {
            return Err(format!("Session {} not found", id));
        }

        Ok(())
    }

    // List all sessions
    pub fn list_sessions(&self) -> Vec<String> {
        // Using try_read to avoid blocking
        match self.sessions.try_read() {
            Ok(sessions) => sessions.keys().cloned().collect(),
            Err(_) => Vec::new(),
        }
    }

    // Get a session's configuration
    pub async fn get_session_config(&self, id: &str) -> Result<SessionConfig, String> {
        let sessions = self.sessions.read().await;

        if let Some(session) = sessions.get(id) {
            Ok(session.config.read().await.clone())
        } else {
            Err(format!("Session {} not found", id))
        }
    }

    // Update a session's configuration
    pub async fn update_session_config<F>(&self, id: &str, update_fn: F) -> Result<(), String>
    where
        F: FnOnce(&mut SessionConfig) -> Result<(), String>,
    {
        let sessions = self.sessions.read().await;

        if let Some(session) = sessions.get(id) {
            let mut config = session.config.write().await;
            update_fn(&mut config)?;
            Ok(())
        } else {
            Err(format!("Session {} not found", id))
        }
    }

    // Process a request through the appropriate session
    pub async fn process_request(
        &self,
        session_id: SessionId,
        req: Request,
    ) -> Result<Response, String> {
        // Find session
        let session = {
            let sessions = self.sessions.read().await;
            sessions.get(&session_id).cloned()
        };

        match session {
            Some(session) => {
                // Update last access time
                let mut last_access = session.last_access.lock().await;
                *last_access = Instant::now();

                // Process request in session
                session.process_request(req).await
            },
            None => Err(format!("Session {} not found", session_id)),
        }
    }
}

// Helper function to check if a header is hop-by-hop
fn is_hop_by_hop_header(header: &str) -> bool {
    matches!(
        header.to_lowercase().as_str(),
        "connection" | "keep-alive" | "proxy-authenticate" | "proxy-authorization" |
        "te" | "trailer" | "transfer-encoding" | "upgrade"
    )
}

impl Session {
    // Process a request in this session
    async fn process_request(
        &self,
        req: Request,
    ) -> Result<Response, String> {
        // Get session config
        let config = self.config.read().await.clone();

        match config.mode {
            SessionMode::Record => {
                self.record_request(req, &config).await
            },
            SessionMode::Replay => self.replay_request(req).await,
        }
    }

    // Record a request and its response
    async fn record_request(
        &self,
        req: Request,
        config: &SessionConfig,
    ) -> Result<Response, String> {
        // Get target URL from the request or config
        let target_url = self.extract_target_url(&req)
            .ok_or_else(|| "No target URL available for request".to_string())?;

        // Process the request and save the interaction
        self.handle_http_request(req, &target_url, true).await
    }

    // Pass through a request without recording
    async fn passthrough_request(
        &self,
        req: Request,
        config: &SessionConfig,
    ) -> Result<Response, String> {
        // Get target URL from the request or config
        let target_url = self.extract_target_url(&req)
            .ok_or_else(|| "No target URL available for request".to_string())?;

        // Process the request without saving
        self.handle_http_request(req, &target_url, false).await
    }

    // Proxy a request
    async fn proxy_request(
        &self,
        req: Request,
    ) -> Result<Response, String> {
        // Extract request parts to check headers
        let (parts, body) = req.into_parts();

        // Get target URL specifically for proxy mode
        let target_url = self.extract_target_url_for_proxy(&parts)
            .ok_or_else(|| "No target URL could be determined for proxy request".to_string())?;

        // Reconstruct the request
        let req = Request::from_parts(parts, body);

        // Process the request without saving
        self.handle_http_request(req, &target_url, false).await
    }

    // Replay a request from stored interactions
    async fn replay_request(
        &self,
        req: Request,
    ) -> Result<Response, String> {
        // Extract the request parts and body
        let (parts, body) = req.into_parts();

        // Read the body bytes
        let body_bytes = to_bytes(body, 1024 * 1024 * 10)
            .await
            .map_err(|e| format!("Failed to read request body: {}", e))?;

        // Reconstruct the request with the bytes body
        let req_with_bytes = Request::builder()
            .method(parts.method)
            .uri(parts.uri)
            .body(body_bytes)
            .map_err(|e| format!("Failed to recreate request with bytes body: {}", e))?;

        // Try to match the request
        let match_result = self.matcher.match_request(&req_with_bytes, &self.id, &self.storage).await
            .map_err(|e| format!("Failed to match request: {}", e))?;

        match match_result {
            MatchResult::Match(resp) => {
                // We found a match, return it
                let (parts, bytes) = resp.into_parts();
                let body = Body::from(bytes);
                let converted_resp = Response::from_parts(parts, body);
                Ok(converted_resp)
            },
            MatchResult::NoMatch => {
                // No match found
                let response = Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::from(Bytes::from("No matching interaction found")))
                    .unwrap();

                Ok(response)
            },
        }
    }

    // Helper method to extract target URL from request and config
    fn extract_target_url(&self, req: &Request) -> Option<String> {
        // Check for X-Proxy-Target header first
        if let Some(target) = req.headers().get("X-Proxy-Target") {
            if let Ok(target_str) = target.to_str() {
                return Some(target_str.to_string());
            }
        }

        // If we have a target_url in config and it's not empty, use it
        let config = match self.config.try_read() {
            Ok(config) => config,
            Err(_) => return None,
        };

        // Try to extract from Host header
        if let Some(host) = req.headers().get("Host") {
            if let Ok(host_str) = host.to_str() {
                // Determine protocol (assume HTTP by default)
                let scheme = req.uri().scheme_str().unwrap_or("http");
                return Some(format!("{}://{}", scheme, host_str));
            }
        }

        // Try to extract from URI
        if let Some(authority) = req.uri().authority() {
            let scheme = req.uri().scheme_str().unwrap_or("http");
            return Some(format!("{}://{}", scheme, authority));
        }

        None
    }

    // Helper specifically for proxy mode
    fn extract_target_url_for_proxy(&self, parts: &axum::http::request::Parts) -> Option<String> {
        // First check if there's a specific header for proxy target
        if let Some(target) = parts.headers.get("X-Proxy-Target") {
            if let Ok(target_str) = target.to_str() {
                return Some(target_str.to_string());
            }
        }

        // Try to get from config
        let config = match self.config.try_read() {
            Ok(config) => config,
            Err(_) => return None,
        };

        // Extract from the Host header
        if let Some(host) = parts.headers.get("Host") {
            if let Ok(host_str) = host.to_str() {
                // Default to HTTP
                return Some(format!("http://{}", host_str));
            }
        }

        // Try to extract from URI
        if let Some(authority) = parts.uri.authority() {
            let scheme = parts.uri.scheme_str().unwrap_or("http");
            return Some(format!("{}://{}", scheme, authority));
        }

        None
    }

    // Unified HTTP/HTTPS request handler
    // Unified HTTP/HTTPS request handler
    async fn handle_http_request(
        &self,
        req: Request,
        target_url: &str,
        save_interaction: bool,
    ) -> Result<Response, String> {
        // Extract parts from the request
        let (parts, body) = req.into_parts();
        let method = parts.method.clone();
        let uri = parts.uri.clone();

        // Read the body bytes
        let body_bytes = to_bytes(body, 1024 * 1024 * 10)
            .await
            .map_err(|e| format!("Failed to read request body: {}", e))?;

        // Construct the forward URL
        let query_str = match uri.query() {
            Some(q) => format!("?{}", q),
            None => String::new()
        };

        let forward_url = format!(
            "{}{}{}",
            target_url,
            uri.path(),
            query_str
        );

        debug!("[Session: {}] Forwarding to URL: {}", self.id, forward_url);

        // Parse the URL
        let target_uri: hyper::Uri = forward_url.parse()
            .map_err(|e| format!("Invalid forward URL: {}", e))?;

        // Create request builder
        let mut request_builder = hyper::Request::builder()
            .method(method.clone())
            .uri(target_uri.clone());

        // Add headers, filtering out session headers and hop-by-hop headers
        for (name, value) in &parts.headers {
            let header_name = name.as_str();
            if !header_name.starts_with("x-session") && !is_hop_by_hop_header(header_name) {
                request_builder = request_builder.header(name, value);
            }
        }

        // Build request with the body
        let hyper_request = request_builder
            .body(Full::new(Bytes::copy_from_slice(&body_bytes)))
            .map_err(|e| format!("Failed to build request: {}", e))?;

        // Create and send request with our client
        debug!("[Session: {}] Sending request to target", self.id);
        let response = self.create_client_and_send_request(hyper_request).await?;

        // Extract status and headers
        let (resp_parts, resp_body) = response.into_parts();
        let status = resp_parts.status;
        let headers = resp_parts.headers;

        debug!("[Session: {}] Received response with status: {}", self.id, status);

        // Read the response body correctly using the frame API
        let mut resp_bytes_vec = Vec::new();
        let mut resp_body = resp_body;

        while let Some(frame) = resp_body.frame().await {
            let frame = frame.map_err(|e| format!("Error reading body frame: {}", e))?;
            if let Some(data) = frame.data_ref() {
                resp_bytes_vec.extend_from_slice(data);
            }
        }

        let resp_bytes = Bytes::from(resp_bytes_vec);

        // If we need to save this interaction for recording
        if save_interaction {
            debug!("[Session: {}] Saving interaction for future replay", self.id);

            // Recreate the request for storage
            let stored_req = Request::builder()
                .method(method)
                .uri(uri)
                .body(Bytes::from(body_bytes))
                .map_err(|e| format!("Failed to recreate request: {}", e))?;

            // Create response for storage
            let stored_resp = Response::builder()
                .status(status.clone())
                .body(Bytes::from(resp_bytes.clone()))
                .map_err(|e| format!("Failed to create response: {}", e))?;

            // Store the interaction
            self.storage.store_interaction(&self.id, &stored_req, &stored_resp)
                .map_err(|e| format!("Failed to store interaction: {}", e))?;
        }

        // Build and return the response
        let mut response_builder = Response::builder().status(status);

        // Add headers to the response
        for (name, value) in headers {
            // Check if the name is Some and not a hop-by-hop header
            if let Some(header_name) = name {
                if !is_hop_by_hop_header(header_name.as_str()) {
                    response_builder = response_builder.header(header_name, value);
                }
            }
        }

        let response = response_builder
            .body(Body::from(resp_bytes))
            .map_err(|e| format!("Failed to build response: {}", e))?;

        Ok(response)
    }

    // Create a client that handles both HTTP and HTTPS
    // Create a client that handles HTTP (without HTTPS for now)
    async fn create_client_and_send_request(
        &self,
        req: hyper::Request<Full<Bytes>>,
    ) -> Result<hyper::Response<Incoming>, String> {
        // Create an HTTP connector
        let mut http = HttpConnector::new();
        http.enforce_http(false); // Allow HTTPS schema in URLs, but will connect over HTTP

        // Log the request target
        debug!("[Session: {}] Sending request to: {}", self.id, req.uri());

        // Create a client with HTTP support only
        let client = hyper_util::client::legacy::Client::builder(TokioExecutor::new())
            .build(http);

        // Send the request
        match client.request(req).await {
            Ok(response) => Ok(response),
            Err(e) => {
                // Log detailed error
                error!("[Session: {}] Failed to send proxy request: {}", self.id, e);
                Err(format!("Failed to send request: {}", e))
            }
        }
    }

    // TODO: Later, we can add proper HTTPS support with appropriate error handling
    // once we resolve the dependency issues or version compatibility.
}