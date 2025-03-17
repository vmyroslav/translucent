use crate::storage::Storage;
use axum::{
    body::Bytes,
    extract::Request,
    response::Response,
};
use log::{debug, info};
use serde_json::Value;
use std::sync::Arc;

// Result of a match operation
pub enum MatchResult {
    Match(Response<Bytes>),
    NoMatch,
}

// Request matcher that handles finding and processing stored interactions
pub struct RequestMatcher {
    // Will hold patterns and matching configuration
}

impl RequestMatcher {
    // Create a new request matcher
    pub fn new() -> Self {
        Self {}
    }

    // Match a request against stored interactions
    pub async fn match_request(
        &self,
        req: &Request<Bytes>,
        session_id: &str,
        storage: &Arc<dyn Storage>,
    ) -> Result<MatchResult, String> {
        // Get method and path
        let method = req.method().clone();
        let path = req.uri().path().to_string();
        let query = req.uri().query().map(|q| q.to_string());

        // For body matching, we'd need a way to check the request body
        // This is a simplified approach for this example

        // Get all interactions for this session
        let interactions = storage.get_interactions(session_id)
            .map_err(|e| format!("Failed to get interactions: {}", e))?;

        debug!("Matching request against {} interactions", interactions.len());

        for (stored_req, response) in interactions {
            // Check if method matches
            if stored_req.method() != req.method() {
                continue;
            }

            // Check if path matches
            if stored_req.uri().path() != req.uri().path() {
                continue;
            }

            // In this simplified version, we match only on method and path
            // A more sophisticated matcher would compare bodies and other elements

            info!("Found matching interaction");
            return Ok(MatchResult::Match(response));
        }

        // No match found
        debug!("No matching interaction found");
        Ok(MatchResult::NoMatch)
    }

    // Check if two JSON values match
    fn json_matches(&self, actual: &Value, expected: &Value) -> bool {
        match (actual, expected) {
            (Value::Object(actual_obj), Value::Object(expected_obj)) => {
                // All keys in expected must be in actual with matching values
                for (key, expected_val) in expected_obj {
                    match actual_obj.get(key) {
                        Some(actual_val) => {
                            if !self.json_matches(actual_val, expected_val) {
                                return false;
                            }
                        },
                        None => return false,
                    }
                }
                true
            },
            (Value::Array(actual_arr), Value::Array(expected_arr)) => {
                // Must have same length and matching items in same order
                if actual_arr.len() != expected_arr.len() {
                    return false;
                }

                for (i, expected_val) in expected_arr.iter().enumerate() {
                    if !self.json_matches(&actual_arr[i], expected_val) {
                        return false;
                    }
                }
                true
            },
            // Special case: wildcard matching
            (_, Value::String(s)) if s == "*" => true,

            // Regular equality for other types
            _ => actual == expected,
        }
    }
}