use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use axum::{
    body::Bytes,
    extract::Request,
    response::Response,
};

// Serializable interaction
#[derive(Serialize, Deserialize)]
pub struct StoredInteraction {
    pub id: String,
    pub timestamp: u64,
    pub request: StoredRequest,
    pub response: StoredResponse,
}

// Serializable request
#[derive(Serialize, Deserialize)]
pub struct StoredRequest {
    pub method: String,
    pub uri: String,
    pub headers: HashMap<String, Vec<String>>,
    pub body: Vec<u8>,
}

// Serializable response
#[derive(Serialize, Deserialize)]
pub struct StoredResponse {
    pub status: u16,
    pub headers: HashMap<String, Vec<String>>,
    pub body: Vec<u8>,
}

// Helper functions for conversion between Axum types and storable types

// Convert Request to StoredRequest
pub fn request_to_stored(request: &Request<Bytes>) -> Result<StoredRequest, String> {
    // Get method and URI
    let method = request.method().to_string();
    let uri = request.uri().to_string();

    // Convert headers
    let mut headers = HashMap::new();
    for (name, value) in request.headers() {
        let name = name.to_string();
        let value = value.to_str()
            .map_err(|_| "Failed to convert header value".to_string())?
            .to_string();

        headers.entry(name)
            .or_insert_with(Vec::new)
            .push(value);
    }

    // Get body bytes
    let body = request.body().to_vec();

    Ok(StoredRequest {
        method,
        uri,
        headers,
        body,
    })
}

// Convert Response to StoredResponse
pub fn response_to_stored(response: &Response<Bytes>) -> Result<StoredResponse, String> {
    // Get status
    let status = response.status().as_u16();

    // Convert headers
    let mut headers = HashMap::new();
    for (name, value) in response.headers() {
        let name = name.to_string();
        let value = value.to_str()
            .map_err(|_| "Failed to convert header value".to_string())?
            .to_string();

        headers.entry(name)
            .or_insert_with(Vec::new)
            .push(value);
    }

    // Get body bytes
    let body = response.body().to_vec();

    Ok(StoredResponse {
        status,
        headers,
        body,
    })
}

// Convert StoredRequest to Request
pub fn stored_to_request(stored: &StoredRequest) -> Result<Request<Bytes>, String> {
    // Create request builder
    let mut builder = Request::builder()
        .method(stored.method.as_str())
        .uri(stored.uri.as_str());

    // Add headers
    for (name, values) in &stored.headers {
        for value in values {
            builder = builder.header(name, value);
        }
    }

    // Build request with body
    builder.body(Bytes::from(stored.body.clone()))
        .map_err(|e| format!("Failed to build request: {}", e))
}

// Convert StoredResponse to Response
pub fn stored_to_response(stored: &StoredResponse) -> Result<Response<Bytes>, String> {
    // Create response builder
    let mut builder = Response::builder()
        .status(stored.status);

    // Add headers
    for (name, values) in &stored.headers {
        for value in values {
            builder = builder.header(name, value);
        }
    }

    // Build response with body
    builder.body(Bytes::from(stored.body.clone()))
        .map_err(|e| format!("Failed to build response: {}", e))
}