use crate::session::{SessionManager, SessionId};
use axum::{
    body::{Bytes, Body, to_bytes},
    extract::{Path, State, Query, Request},
    http::{StatusCode, HeaderMap, Uri, Method},
    response::{IntoResponse, Response},
    Json,
};
use log::{info, error};
use serde::{Serialize, Deserialize};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

// Query parameters for session extraction
#[derive(Debug, Deserialize)]
pub struct SessionQuery {
    pub session: Option<String>,
}

// Session create payload
#[derive(Debug, Deserialize)]
pub struct CreateSessionPayload {
    pub session_id: String,
}

// App state to share session manager
#[derive(Clone)]
pub struct AppState {
    pub session_manager: Arc<SessionManager>,
}

// Get server information handler
pub async fn get_server_info(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let info = json!({
        "version": env!("CARGO_PKG_VERSION"),
        "sessions": state.session_manager.get_session_count(),
    });

    Json(info)
}

// List all sessions handler
pub async fn list_sessions(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let sessions = state.session_manager.list_sessions();
    Json(sessions)
}

// Create a new session handler
pub async fn create_session(
    State(state): State<AppState>,
    Json(payload): Json<CreateSessionPayload>,
) -> impl IntoResponse {
    if payload.session_id.is_empty() {
        return (StatusCode::BAD_REQUEST, "Missing session_id field").into_response();
    }

    match state.session_manager.create_session(payload.session_id).await {
        Ok(_) => (StatusCode::CREATED, "Session created").into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {}", err)).into_response(),
    }
}

// Delete a session handler
pub async fn delete_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.session_manager.delete_session(&id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {}", err)).into_response(),
    }
}

// Extract session ID from request
fn extract_session_id(headers: &HeaderMap, query: &SessionQuery) -> SessionId {
    // Try to get from header
    if let Some(header) = headers.get("X-Session-Id") {
        if let Ok(session_id) = header.to_str() {
            return session_id.to_string();
        }
    }

    // Try to get from query parameter
    if let Some(session) = &query.session {
        return session.clone();
    }

    // Fall back to default session
    "default".to_string()
}

// Main API request handler
pub async fn handle_api_request(
    State(state): State<AppState>,
    req: Request,
) -> impl IntoResponse {
    // Extract headers and query params from the request
    let headers = req.headers().clone();
    let uri = req.uri().clone();

    // Parse query parameters
    let query_params = req.uri().query()
        .map(|q| {
            serde_qs::from_str::<SessionQuery>(q)
                .unwrap_or_else(|_| SessionQuery { session: None })
        })
        .unwrap_or_else(|| SessionQuery { session: None });

    // Extract session ID (always returns a valid session ID now)
    let session_id = extract_session_id(&headers, &query_params);

    // Ensure session exists
    if !state.session_manager.session_exists(&session_id).await {
        match state.session_manager.create_session(session_id.clone()).await {
            Ok(_) => info!("Auto-created session: {}", session_id),
            Err(err) => {
                error!("Failed to auto-create session: {}", err);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to create session: {}", err),
                ).into_response();
            }
        }
    }

    // Process the request through the appropriate session
    match state.session_manager.process_request(session_id, req).await {
        Ok(response) => response.into_response(),
        Err(err) => {
            error!("Error processing request: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Error: {}", err),
            ).into_response()
        }
    }
}