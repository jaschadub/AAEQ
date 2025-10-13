/// Route handlers for the Control API

use super::types::*;
use crate::manager::OutputManager;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub manager: Arc<RwLock<OutputManager>>,
    pub metrics: Arc<RwLock<Metrics>>,
    pub route_config: Arc<RwLock<RouteConfig>>,
}

/// Metrics tracking
#[derive(Debug, Clone, Default)]
pub struct Metrics {
    pub underruns: u64,
    pub overruns: u64,
    pub bytes_written: u64,
}

/// Current routing configuration
#[derive(Debug, Clone, Default)]
pub struct RouteConfig {
    pub input: Option<String>,
    pub output: Option<String>,
    pub device: Option<String>,
}

/// Create the API router
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/v1/outputs", get(list_outputs))
        .route("/v1/outputs/select", post(select_output))
        .route("/v1/outputs/start", post(start_output))
        .route("/v1/outputs/stop", post(stop_output))
        .route("/v1/outputs/metrics", get(get_metrics))
        .route("/v1/route", get(get_route).post(set_route))
        .route("/v1/capabilities", get(get_capabilities))
        .route("/v1/health", get(health_check))
        .with_state(state)
}

/// GET /v1/outputs - List all output sinks and their status
async fn list_outputs(State(state): State<AppState>) -> Response {
    debug!("GET /v1/outputs");

    let manager = state.manager.read().await;
    let sink_names = manager.list_sinks();
    let active_name = manager.active_sink_name();

    let mut outputs = Vec::new();
    for name in sink_names.iter() {
        // Get sink info - in a real implementation, we'd need to enhance OutputManager
        // to expose more details about each sink
        let is_active = active_name == Some(name);

        outputs.push(OutputInfo {
            name: name.to_string(),
            is_open: false, // Would need to track this in manager
            is_active,
            config: None,   // Would need to store configs
            latency_ms: 0,  // Would need to query sink
        });
    }

    let response = OutputsResponse {
        outputs,
        active: active_name.map(|s| s.to_string()),
    };

    Json(response).into_response()
}

/// POST /v1/outputs/select - Select and configure an output sink
async fn select_output(
    State(state): State<AppState>,
    Json(req): Json<SelectOutputRequest>,
) -> Response {
    info!("POST /v1/outputs/select: {}", req.name);

    let mut manager = state.manager.write().await;

    // Try to select the sink by name
    let result = manager.select_sink_by_name(&req.name, req.config).await;

    match result {
        Ok(_) => {
            let response = SelectOutputResponse {
                success: true,
                message: format!("Successfully selected output: {}", req.name),
                active_output: Some(req.name.clone()),
            };

            // Update route config if this is part of routing
            if let Some(device) = req.device {
                let mut route = state.route_config.write().await;
                route.output = Some(req.name);
                route.device = Some(device);
            }

            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            error!("Failed to select output: {}", e);
            let response = ErrorResponse {
                error: "Failed to select output".to_string(),
                details: Some(e.to_string()),
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response()
        }
    }
}

/// POST /v1/outputs/start - Start the active output (if not already streaming)
async fn start_output(State(_state): State<AppState>) -> Response {
    info!("POST /v1/outputs/start");

    // In a real implementation, this would start audio streaming
    // For now, we return success as opening the sink starts it
    let response = SuccessResponse {
        success: true,
        message: "Output already started (started on select)".to_string(),
    };

    Json(response).into_response()
}

/// POST /v1/outputs/stop - Stop the active output
async fn stop_output(State(state): State<AppState>) -> Response {
    info!("POST /v1/outputs/stop");

    let mut manager = state.manager.write().await;

    match manager.close_active().await {
        Ok(_) => {
            let response = SuccessResponse {
                success: true,
                message: "Output stopped successfully".to_string(),
            };
            Json(response).into_response()
        }
        Err(e) => {
            error!("Failed to stop output: {}", e);
            let response = ErrorResponse {
                error: "Failed to stop output".to_string(),
                details: Some(e.to_string()),
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response()
        }
    }
}

/// GET /v1/outputs/metrics - Get metrics for the active output
async fn get_metrics(State(state): State<AppState>) -> Response {
    debug!("GET /v1/outputs/metrics");

    let manager = state.manager.read().await;
    let metrics = state.metrics.read().await;

    let output_name = manager.active_sink_name().map(|s| s.to_string());

    let response = MetricsResponse {
        output_name,
        sample_rate: None, // Would need to store active config
        channels: None,
        format: None,
        latency_ms: 0, // Would need to query active sink
        underruns: metrics.underruns,
        overruns: metrics.overruns,
        bytes_written: metrics.bytes_written,
    };

    Json(response).into_response()
}

/// GET /v1/route - Get current routing configuration
async fn get_route(State(state): State<AppState>) -> Response {
    debug!("GET /v1/route");

    let route = state.route_config.read().await;
    let manager = state.manager.read().await;

    let response = RouteResponse {
        input: route.input.clone(),
        output: route.output.clone(),
        device: route.device.clone(),
        is_active: manager.active_sink_name().is_some(),
    };

    Json(response).into_response()
}

/// POST /v1/route - Set routing configuration
async fn set_route(
    State(state): State<AppState>,
    Json(req): Json<RouteRequest>,
) -> Response {
    info!("POST /v1/route: {} -> {}", req.input, req.output);

    // Update route configuration
    {
        let mut route = state.route_config.write().await;
        route.input = Some(req.input.clone());
        route.output = Some(req.output.clone());
        route.device = req.device.clone();
    }

    // If config provided, select the output
    if let Some(config) = req.config {
        let mut manager = state.manager.write().await;

        match manager.select_sink_by_name(&req.output, config).await {
            Ok(_) => {
                let response = SuccessResponse {
                    success: true,
                    message: format!("Route set: {} -> {}", req.input, req.output),
                };
                Json(response).into_response()
            }
            Err(e) => {
                error!("Failed to set route: {}", e);
                let response = ErrorResponse {
                    error: "Failed to set route".to_string(),
                    details: Some(e.to_string()),
                };
                (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response()
            }
        }
    } else {
        // Just update routing config without activating
        let response = SuccessResponse {
            success: true,
            message: "Route configuration updated".to_string(),
        };
        Json(response).into_response()
    }
}

/// GET /v1/capabilities - Get supported capabilities for each output type
async fn get_capabilities(State(_state): State<AppState>) -> Response {
    debug!("GET /v1/capabilities");

    let capabilities = vec![
        OutputCapability::for_local_dac(),
        OutputCapability::for_dlna(),
        OutputCapability::for_airplay(),
    ];

    let response = CapabilitiesResponse {
        outputs: capabilities,
    };

    Json(response).into_response()
}

/// GET /v1/health - Health check endpoint
async fn health_check() -> Response {
    let response = serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
    });

    Json(response).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_capability_creation() {
        let dac = OutputCapability::for_local_dac();
        assert_eq!(dac.name, "local_dac");
        assert!(dac.supports_exclusive);
        assert!(!dac.requires_device_discovery);

        let dlna = OutputCapability::for_dlna();
        assert_eq!(dlna.name, "dlna");
        assert!(!dlna.supports_exclusive);
        assert!(dlna.requires_device_discovery);

        let airplay = OutputCapability::for_airplay();
        assert_eq!(airplay.name, "airplay");
        assert!(airplay.requires_device_discovery);
    }
}
