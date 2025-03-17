use crate::config::StorageConfig;
use crate::storage::{Storage, MemoryStorage, FileSystemStorage};
use std::sync::Arc;

// Factory for creating storage implementations
pub struct StorageFactory;

impl StorageFactory {
    // Create a storage implementation based on config
    pub fn create_storage(config: &StorageConfig) -> Result<Arc<dyn Storage>, String> {
        match config.type_.as_str() {
            "memory" => Ok(Arc::new(MemoryStorage::new())),
            "filesystem" => Ok(Arc::new(FileSystemStorage::new(&config.path)?)),
            _ => Err(format!("Unknown storage type: {}", config.type_)),
        }
    }
}