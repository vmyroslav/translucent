mod memory;
mod filesystem;
mod factory;
mod models;

pub use models::*;
pub use factory::StorageFactory;
pub use memory::MemoryStorage;
pub use filesystem::FileSystemStorage;

// Storage trait for different backends
pub trait Storage: Send + Sync {
    fn store_interaction(
        &self,
        session_id: &str,
        request: &axum::extract::Request<axum::body::Bytes>,
        response: &axum::response::Response<axum::body::Bytes>
    ) -> Result<(), String>;

    fn get_interactions(
        &self,
        session_id: &str
    ) -> Result<Vec<(axum::extract::Request<axum::body::Bytes>, axum::response::Response<axum::body::Bytes>)>, String>;

    fn clear_interactions(&self, session_id: &str) -> Result<(), String>;
}