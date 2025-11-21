//! # Sideways 🦀
//!
//! > *Observability from the side - because crabs walk sideways, and so should your telemetry.*
//!
//! A production-ready telemetry library for Rust services that provides:
//! - **Datadog tracing** via dd-trace-rs with OpenTelemetry
//! - **StatsD metrics** via Cadence with production-ready setup
//!
//! ## Features
//!
//! - Easy one-line initialization for both tracing and metrics
//! - Graceful degradation when services are unavailable
//! - Environment-based configuration
//! - Health check filtering to reduce noise
//! - Convenient prelude module with all metrics macros
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use sideways::{init_telemetry, TelemetryConfig};
//! use sideways::prelude::*;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Initialize both Datadog tracing and StatsD metrics
//!     let config = TelemetryConfig::from_env();
//!     let telemetry = init_telemetry(config).await;
//!
//!     // Use tracing as normal
//!     tracing::info!("Application started");
//!
//!     // Emit metrics using macros - no need to import cadence!
//!     statsd_count!("requests.handled", 1, "status" => "success");
//!
//!     // Cleanup on shutdown
//!     if let Some(tracer) = telemetry.tracer_provider {
//!         let _ = tracer.shutdown();
//!     }
//! }
//! ```

pub mod metrics;
pub mod prelude;
pub mod tracing;

// Re-export cadence and cadence-macros for advanced usage
pub use cadence;
pub use cadence_macros;

use std::env;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TelemetryError {
    #[error("Datadog tracing disabled via DD_TRACE_ENABLED=false")]
    DatadogDisabled,

    #[error("Failed to set global subscriber: {0}")]
    SubscriberInit(String),

    #[error("Metrics disabled via METRICS_ENABLED=false")]
    MetricsDisabled,

    #[error("Failed to bind UDP socket: {0}")]
    SocketBind(std::io::Error),

    #[error("Failed to create metric sink: {0}")]
    SinkCreation(cadence::MetricError),
}

/// Configuration for telemetry initialization
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// Enable/disable Datadog tracing (default: true)
    pub datadog_enabled: bool,
    /// Datadog service name
    pub dd_service: String,
    /// Datadog environment
    pub dd_env: String,
    /// Datadog trace agent URL
    pub dd_trace_agent_url: String,
    /// RUST_LOG filter
    pub rust_log: String,

    /// Enable/disable metrics (default: true)
    pub metrics_enabled: bool,
    /// StatsD host
    pub statsd_host: String,
    /// StatsD port
    pub statsd_port: u16,
    /// Metrics prefix/namespace
    pub metrics_prefix: String,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            datadog_enabled: true,
            dd_service: "sideways-service".to_string(),
            dd_env: "development".to_string(),
            dd_trace_agent_url: "http://localhost:8126".to_string(),
            rust_log: "info".to_string(),
            metrics_enabled: true,
            statsd_host: "localhost".to_string(),
            statsd_port: 8125,
            metrics_prefix: "sideways".to_string(),
        }
    }
}

impl TelemetryConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // Check if Datadog is explicitly disabled
        if let Ok(enabled) = env::var("DD_TRACE_ENABLED") {
            if enabled.to_lowercase() == "false" {
                config.datadog_enabled = false;
            }
        }

        // Check if metrics are explicitly disabled
        if let Ok(enabled) = env::var("METRICS_ENABLED") {
            if enabled.to_lowercase() == "false" {
                config.metrics_enabled = false;
            }
        }

        // Datadog configuration
        if let Ok(service) = env::var("DD_SERVICE") {
            config.dd_service = service;
        }
        if let Ok(dd_env) = env::var("DD_ENV") {
            config.dd_env = dd_env;
        }
        if let Ok(url) = env::var("DD_TRACE_AGENT_URL") {
            config.dd_trace_agent_url = url;
        }

        // Logging configuration
        if let Ok(rust_log) = env::var("RUST_LOG") {
            config.rust_log = rust_log;
        }

        // Metrics configuration
        if let Ok(host) = env::var("STATSD_HOST") {
            config.statsd_host = host;
        }
        if let Ok(port) = env::var("STATSD_PORT") {
            if let Ok(port_num) = port.parse() {
                config.statsd_port = port_num;
            }
        }
        if let Ok(prefix) = env::var("METRICS_PREFIX") {
            config.metrics_prefix = prefix;
        }

        config
    }

    /// Create a builder for custom configuration
    pub fn builder() -> TelemetryConfigBuilder {
        TelemetryConfigBuilder::default()
    }
}

/// Builder for TelemetryConfig
#[derive(Debug, Default)]
pub struct TelemetryConfigBuilder {
    config: TelemetryConfig,
}

impl TelemetryConfigBuilder {
    pub fn datadog_enabled(mut self, enabled: bool) -> Self {
        self.config.datadog_enabled = enabled;
        self
    }

    pub fn dd_service(mut self, service: impl Into<String>) -> Self {
        self.config.dd_service = service.into();
        self
    }

    pub fn dd_env(mut self, env: impl Into<String>) -> Self {
        self.config.dd_env = env.into();
        self
    }

    pub fn dd_trace_agent_url(mut self, url: impl Into<String>) -> Self {
        self.config.dd_trace_agent_url = url.into();
        self
    }

    pub fn rust_log(mut self, filter: impl Into<String>) -> Self {
        self.config.rust_log = filter.into();
        self
    }

    pub fn metrics_enabled(mut self, enabled: bool) -> Self {
        self.config.metrics_enabled = enabled;
        self
    }

    pub fn statsd_host(mut self, host: impl Into<String>) -> Self {
        self.config.statsd_host = host.into();
        self
    }

    pub fn statsd_port(mut self, port: u16) -> Self {
        self.config.statsd_port = port;
        self
    }

    pub fn metrics_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.config.metrics_prefix = prefix.into();
        self
    }

    pub fn build(self) -> TelemetryConfig {
        self.config
    }
}

/// Telemetry components that need to be kept alive
pub struct Telemetry {
    /// Datadog tracer provider (must be kept alive and shutdown on exit)
    pub tracer_provider: Option<opentelemetry_sdk::trace::SdkTracerProvider>,
}

/// Initialize telemetry with the given configuration.
///
/// This will:
/// 1. Initialize Datadog tracing (if enabled)
/// 2. Initialize StatsD metrics (if enabled)
/// 3. Set up console logging
///
/// Returns a `Telemetry` struct that must be kept alive for the duration
/// of the application and properly shutdown on exit.
pub async fn init_telemetry(config: TelemetryConfig) -> Telemetry {
    eprintln!("🦀 Sideways Telemetry: Initializing...");

    // Initialize Datadog tracing
    let tracer_provider = if config.datadog_enabled {
        match tracing::init_datadog(&config) {
            Ok(provider) => {
                eprintln!("✅ Sideways Telemetry: Datadog tracing initialized");
                Some(provider)
            }
            Err(err) => {
                eprintln!("⚠️  Sideways Telemetry: Datadog tracing unavailable: {}", err);
                None
            }
        }
    } else {
        eprintln!("📊 Sideways Telemetry: Datadog tracing disabled");
        tracing::init_console_logging(&config);
        None
    };

    // Initialize metrics
    if config.metrics_enabled {
        if let Err(err) = metrics::init_metrics(&config) {
            eprintln!("⚠️  Sideways Telemetry: Metrics unavailable: {}", err);
        }
    } else {
        eprintln!("📊 Sideways Telemetry: Metrics disabled");
    }

    Telemetry { tracer_provider }
}
