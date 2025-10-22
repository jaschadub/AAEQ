/// Control API Server implementation
use super::routes::{create_router, AppState, Metrics, RouteConfig};
use crate::manager::OutputManager;
use anyhow::Result;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{info, error};

/// HTTP Control API Server
pub struct ControlServer {
    addr: SocketAddr,
    manager: Arc<RwLock<OutputManager>>,
    server_handle: Option<JoinHandle<()>>,
}

impl ControlServer {
    /// Create a new control server
    ///
    /// # Arguments
    /// * `addr` - Address to bind to (e.g., "127.0.0.1:8080")
    /// * `manager` - Shared output manager
    pub fn new(addr: SocketAddr, manager: Arc<RwLock<OutputManager>>) -> Self {
        Self {
            addr,
            manager,
            server_handle: None,
        }
    }

    /// Start the control server
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting Control API server on {}", self.addr);

        let state = AppState {
            manager: self.manager.clone(),
            metrics: Arc::new(RwLock::new(Metrics::default())),
            route_config: Arc::new(RwLock::new(RouteConfig::default())),
        };

        let app = create_router(state);

        let listener = tokio::net::TcpListener::bind(self.addr).await?;
        info!("Control API listening on {}", self.addr);

        // Spawn server task
        let handle = tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                error!("Control API server error: {}", e);
            }
        });

        self.server_handle = Some(handle);

        Ok(())
    }

    /// Stop the control server
    pub async fn stop(&mut self) {
        if let Some(handle) = self.server_handle.take() {
            info!("Stopping Control API server");
            handle.abort();
        }
    }

    /// Get the server address
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for ControlServer {
    fn drop(&mut self) {
        if let Some(handle) = self.server_handle.take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_control_server_creation() {
        let manager = Arc::new(RwLock::new(OutputManager::new()));
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let server = ControlServer::new(addr, manager);

        assert_eq!(server.addr(), addr);
    }
}
