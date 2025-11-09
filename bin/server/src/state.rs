use std::sync::Arc;

/// Server application state
pub struct AppState {
    pub storage: Arc<dyn storage::Storage>,
}

impl AppState {
    pub fn new(storage: Arc<dyn storage::Storage>) -> Self {
        Self { storage }
    }
}
