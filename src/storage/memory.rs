use crate::storage::{Storage, StoredInteraction, request_to_stored, response_to_stored, stored_to_request, stored_to_response};
use axum::{
    body::Bytes,
    extract::Request,
    response::Response,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

// Memory-based storage
pub struct MemoryStorage {
    interactions: Arc<Mutex<HashMap<String, Vec<StoredInteraction>>>>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            interactions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Storage for MemoryStorage {
    fn store_interaction(
        &self,
        session_id: &str,
        request: &Request<Bytes>,
        response: &Response<Bytes>,
    ) -> Result<(), String> {
        // Convert request to storable format
        let stored_request = request_to_stored(request)
            .map_err(|e| format!("Failed to convert request: {}", e))?;

        // Convert response to storable format
        let stored_response = response_to_stored(response)
            .map_err(|e| format!("Failed to convert response: {}", e))?;

        // Create interaction
        let interaction = StoredInteraction {
            id: Uuid::new_v4().to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            request: stored_request,
            response: stored_response,
        };

        // Store in memory
        let mut interactions = self.interactions.lock()
            .map_err(|e| format!("Failed to lock interactions: {}", e))?;

        let session_interactions = interactions
            .entry(session_id.to_string())
            .or_insert_with(Vec::new);

        session_interactions.push(interaction);

        Ok(())
    }

    fn get_interactions(
        &self,
        session_id: &str,
    ) -> Result<Vec<(Request<Bytes>, Response<Bytes>)>, String> {
        let interactions = self.interactions.lock()
            .map_err(|e| format!("Failed to lock interactions: {}", e))?;

        let session_interactions = match interactions.get(session_id) {
            Some(interactions) => interactions,
            None => return Ok(Vec::new()),
        };

        let mut result = Vec::new();

        for interaction in session_interactions {
            // Convert stored request to Request
            let request = stored_to_request(&interaction.request)
                .map_err(|e| format!("Failed to convert request: {}", e))?;

            // Convert stored response to Response
            let response = stored_to_response(&interaction.response)
                .map_err(|e| format!("Failed to convert response: {}", e))?;

            result.push((request, response));
        }

        Ok(result)
    }

    fn clear_interactions(&self, session_id: &str) -> Result<(), String> {
        let mut interactions = self.interactions.lock()
            .map_err(|e| format!("Failed to lock interactions: {}", e))?;

        interactions.remove(session_id);

        Ok(())
    }
}