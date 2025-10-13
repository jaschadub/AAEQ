/// HTTP Control API for Stream Server
///
/// Provides REST endpoints for controlling audio output routing and configuration

pub mod routes;
pub mod server;
pub mod types;

pub use server::ControlServer;
pub use types::*;
