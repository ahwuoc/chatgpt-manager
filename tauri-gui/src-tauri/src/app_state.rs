use std::sync::{Arc, Mutex};

pub struct AppState {
    pub running_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}
