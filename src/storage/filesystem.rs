use crate::storage::{Storage, StoredInteraction, request_to_stored, response_to_stored, stored_to_request, stored_to_response};
use axum::{
    body::Bytes,
    extract::Request,
    response::Response,
};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{PathBuf};
use uuid::Uuid;

// File system-based storage
pub struct FileSystemStorage {
    base_path: PathBuf,
}

impl FileSystemStorage {
    pub fn new(base_path: &str) -> Result<Self, String> {
        let path = PathBuf::from(base_path);

        // Create directory if it doesn't exist
        if !path.exists() {
            fs::create_dir_all(&path)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        Ok(Self {
            base_path: path,
        })
    }

    // Get path for a session
    fn get_session_path(&self, session_id: &str) -> PathBuf {
        let mut path = self.base_path.clone();
        path.push(session_id);
        path
    }

    // Get path for an interaction
    fn get_interaction_path(
        &self,
        session_id: &str,
        interaction_id: &str,
    ) -> PathBuf {
        let mut path = self.get_session_path(session_id);
        path.push(format!("{}.json", interaction_id));
        path
    }
}

impl Storage for FileSystemStorage {
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

        // Create session directory if it doesn't exist
        let session_path = self.get_session_path(session_id);
        if !session_path.exists() {
            fs::create_dir_all(&session_path)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        // Serialize and write to file
        let interaction_path = self.get_interaction_path(session_id, &interaction.id);
        let json = serde_json::to_string_pretty(&interaction)
            .map_err(|e| format!("Failed to serialize interaction: {}", e))?;

        let mut file = File::create(interaction_path)
            .map_err(|e| format!("Failed to create file: {}", e))?;

        file.write_all(json.as_bytes())
            .map_err(|e| format!("Failed to write to file: {}", e))?;

        Ok(())
    }

    fn get_interactions(
        &self,
        session_id: &str,
    ) -> Result<Vec<(Request<Bytes>, Response<Bytes>)>, String> {
        let session_path = self.get_session_path(session_id);

        // If directory doesn't exist, return empty list
        if !session_path.exists() {
            return Ok(Vec::new());
        }

        let mut result = Vec::new();

        // Read all files in the directory
        let entries = fs::read_dir(&session_path)
            .map_err(|e| format!("Failed to read directory: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();

            // Skip non-JSON files
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }

            // Read file
            let mut file = File::open(&path)
                .map_err(|e| format!("Failed to open file: {}", e))?;

            let mut contents = String::new();
            file.read_to_string(&mut contents)
                .map_err(|e| format!("Failed to read file: {}", e))?;

            // Deserialize
            let interaction: StoredInteraction = serde_json::from_str(&contents)
                .map_err(|e| format!("Failed to deserialize interaction: {}", e))?;

            // Convert to Request and Response
            let request = stored_to_request(&interaction.request)
                .map_err(|e| format!("Failed to convert request: {}", e))?;

            let response = stored_to_response(&interaction.response)
                .map_err(|e| format!("Failed to convert response: {}", e))?;

            result.push((request, response));
        }

        Ok(result)
    }

    fn clear_interactions(&self, session_id: &str) -> Result<(), String> {
        let session_path = self.get_session_path(session_id);

        // If directory doesn't exist, nothing to do
        if !session_path.exists() {
            return Ok(());
        }

        // Remove directory and all contents
        fs::remove_dir_all(&session_path)
            .map_err(|e| format!("Failed to remove directory: {}", e))?;

        Ok(())
    }
}