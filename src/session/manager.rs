use crate::matching::{RequestMatcher, MatchResult};
use crate::storage::Storage;
use crate::session::{SessionId, SessionConfig, SessionMode};

use axum::{
    body::{Bytes, Body, to_bytes},
    extract::Request,
    response::{Response},
    http::StatusCode,
};

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{RwLock, Mutex};
use hyper_util::{
    rt::TokioExecutor,
    client::legacy::connect::HttpConnector,
};
use http_body_util::{BodyExt, Full};

// Session manager that handles multiple sessions
pub struct SessionManager {
    storage: Arc<dyn Storage>,
    sessions: RwLock<HashMap<SessionId, Arc<Session>>>,
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
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self {
            storage,
            sessions: RwLock::new(HashMap::new()),
        }
    }

    // Get current session count
    pub fn get_session_count(&self) -> usize {
        // This is not real-time accurate but good enough for info purposes
        // Using try_read to avoid blocking in case of contention
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

        // Default session config
        let config = SessionConfig::default();

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
            SessionMode::Passthrough => {
                self.passthrough_request(req, &config).await
            },
        }
    }

    // Record a request and its response
    async fn record_request(
        &self,
        req: Request,
        config: &SessionConfig,
    ) -> Result<Response, String> {
        // We need a target URL to forward to
        let target_url = match &config.target_url {
            Some(url) => url,
            None => return Err("No target URL configured for recording".to_string()),
        };

        // Extract parts from the request
        let (parts, body) = req.into_parts();
        let method = parts.method.clone();

        // Read the body bytes
        let body_bytes = to_bytes(body, 1024 * 1024 * 10)
            .await
            .map_err(|e| format!("Failed to read request body: {}", e))?;

        // Create a hyper client to forward the request
        let client = hyper_util::client::legacy::Client::builder(TokioExecutor::new())
            .build(HttpConnector::new());

        // Construct the forward URL
        let query_str = match parts.uri.query() {
            Some(q) => format!("?{}", q),
            None => String::new()
        };

        let forward_url = format!(
            "{}{}{}",
            target_url,
            parts.uri.path(),
            query_str
        );

        // Parse the URL
        let uri: hyper::Uri = forward_url.parse()
            .map_err(|e| format!("Invalid forward URL: {}", e))?;

        // Create request builder
        let mut request_builder = hyper::Request::builder()
            .method(method)
            .uri(uri);

        // Add headers
        for (name, value) in parts.headers {
            if let Some(name_str) = name {
                if !name_str.as_str().starts_with("x-session") {
                    request_builder = request_builder.header(name_str, value);
                }
            }
        }

        // Build request with the body
        let forwarded_req = request_builder
            .body(Full::new(body_bytes.clone()))
            .map_err(|e| format!("Failed to build request: {}", e))?;

        // Send the request
        let resp = client.request(forwarded_req)
            .await
            .map_err(|e| format!("Failed to forward request: {}", e))?;

        // Extract status and headers
        let status = resp.status();
        let headers = resp.headers().clone();

        // Read the response body
        let mut resp_body = resp.into_body();
        let mut resp_bytes = Vec::new();

        while let Some(chunk) = resp_body.frame().await {
            let chunk = chunk.map_err(|e| format!("Failed to read response chunk: {}", e))?;
            if let Some(data) = chunk.data_ref() {
                resp_bytes.extend_from_slice(data);
            }
        }

        let resp_bytes = Bytes::from(resp_bytes);

        // Recreate the request for storage with Bytes body
        let stored_req = Request::builder()
            .method(parts.method)
            .uri(parts.uri)
            .body(body_bytes.clone())
            .map_err(|e| format!("Failed to recreate request: {}", e))?;

        // Create response for storage with Bytes body
        let stored_resp = Response::builder()
            .status(status)
            .body(resp_bytes.clone())
            .map_err(|e| format!("Failed to create response: {}", e))?;

        // Store the interaction
        self.storage.store_interaction(&self.id, &stored_req, &stored_resp)
            .map_err(|e| format!("Failed to store interaction: {}", e))?;

        // Build and return the response
        let mut response_builder = Response::builder().status(status);

        // Add headers to the response
        for (name, value) in headers {
            if let Some(name) = name {
                response_builder = response_builder.header(name, value);
            }
        }

        let response = response_builder
            .body(Body::from(resp_bytes))  // Convert Bytes to Body
            .map_err(|e| format!("Failed to build response: {}", e))?;

        Ok(response)
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

    // Pass through a request without recording
    async fn passthrough_request(
        &self,
        req: Request,
        config: &SessionConfig,
    ) -> Result<Response, String> {
        // We need a target URL to forward to
        let target_url = match &config.target_url {
            Some(url) => url,
            None => return Err("No target URL configured for passthrough".to_string()),
        };

        // Extract parts from the request
        let (parts, body) = req.into_parts();
        let method = parts.method.clone();

        // Read the body bytes
        let body_bytes = to_bytes(body, 1024 * 1024 * 10)
            .await
            .map_err(|e| format!("Failed to read request body: {}", e))?;

        // Create a hyper client to forward the request
        let client = hyper_util::client::legacy::Client::builder(TokioExecutor::new())
            .build(HttpConnector::new());

        // Construct the forward URL
        let query_str = match parts.uri.query() {
            Some(q) => format!("?{}", q),
            None => String::new()
        };

        let forward_url = format!(
            "{}{}{}",
            target_url,
            parts.uri.path(),
            query_str
        );

        // Parse the URL
        let uri: hyper::Uri = forward_url.parse()
            .map_err(|e| format!("Invalid forward URL: {}", e))?;

        // Create request builder
        let mut request_builder = hyper::Request::builder()
            .method(method)
            .uri(uri);

        // Add headers
        for (name, value) in parts.headers {
            if let Some(name_str) = name {
                if !name_str.as_str().starts_with("x-session") {
                    request_builder = request_builder.header(name_str, value);
                }
            }
        }

        // Build request with the body
        let forwarded_req = request_builder
            .body(Full::new(body_bytes.clone()))
            .map_err(|e| format!("Failed to build request: {}", e))?;

        // Send the request
        let resp = client.request(forwarded_req)
            .await
            .map_err(|e| format!("Failed to forward request: {}", e))?;

        // Extract status and headers
        let status = resp.status();
        let headers = resp.headers().clone();

        // Read the response body
        let mut resp_body = resp.into_body();
        let mut resp_bytes = Vec::new();

        while let Some(chunk) = resp_body.frame().await {
            let chunk = chunk.map_err(|e| format!("Failed to read response chunk: {}", e))?;
            if let Some(data) = chunk.data_ref() {
                resp_bytes.extend_from_slice(data);
            }
        }

        let resp_bytes = Bytes::from(resp_bytes);

        // Build and return the response
        let mut response_builder = Response::builder().status(status);

        // Add headers to the response
        for (name, value) in headers {
            if let Some(name) = name {
                response_builder = response_builder.header(name, value);
            }
        }

        let response = response_builder
            .body(Body::from(resp_bytes))
            .map_err(|e| format!("Failed to build response: {}", e))?;

        Ok(response)
    }
}