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
    /// Global tags to append to all metrics
    pub global_tags: Vec<(String, String)>,

    /// Enable/disable Datadog log ingestion (default: true)
    pub dd_logs_enabled: bool,
    /// Enable JSON-formatted console logging (default: false)
    pub json_logging: bool,

    /// Use synchronous (non-queued) metric dispatch.
    ///
    /// When true, each `statsd_*!` call sends a UDP packet immediately and
    /// blocks until the send completes, rather than handing off to a
    /// background thread. This is automatically enabled when
    /// `AWS_LAMBDA_FUNCTION_NAME` is set, because Lambda freezes the execution
    /// environment the moment the handler returns — the `QueuingMetricSink`
    /// background thread would be frozen before flushing, causing metrics to
    /// be dropped or deferred to the next invocation.
    ///
    /// For long-running services (ECS, Kubernetes, etc.) leave this `false`
    /// to keep the async, batched behaviour.
    pub sync_metrics: bool,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        // Auto-detect Lambda: AWS sets AWS_LAMBDA_FUNCTION_NAME on every Lambda.
        let sync_metrics = env::var("AWS_LAMBDA_FUNCTION_NAME").is_ok();
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
            global_tags: Vec::new(),
            dd_logs_enabled: true,
            json_logging: false,
            sync_metrics,
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
        if let Ok(tags_str) = env::var("STATSD_GLOBAL_TAGS") {
            config.global_tags = Self::parse_tags(&tags_str);
        }
        // Automatically include DD_ENV as "env" tag if not already set via STATSD_GLOBAL_TAGS
        if !config.global_tags.iter().any(|(k, _)| k == "env") {
            config
                .global_tags
                .push(("env".to_string(), config.dd_env.clone()));
        }

        // Datadog logs configuration
        if let Ok(enabled) = env::var("DD_LOGS_ENABLED") {
            if enabled.to_lowercase() == "false" {
                config.dd_logs_enabled = false;
            }
        }

        // JSON logging configuration
        if let Ok(enabled) = env::var("JSON_LOGGING") {
            if enabled.to_lowercase() == "true" {
                config.json_logging = true;
            }
        }

        config
    }

    /// Parse tags from a string in the format "key1:value1,key2:value2"
    fn parse_tags(tags_str: &str) -> Vec<(String, String)> {
        tags_str
            .split(',')
            .filter_map(|tag| {
                let parts: Vec<&str> = tag.trim().splitn(2, ':').collect();
                if parts.len() == 2 {
                    Some((parts[0].to_string(), parts[1].to_string()))
                } else {
                    None
                }
            })
            .collect()
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

    /// Set global tags that will be appended to all metrics
    pub fn global_tags(mut self, tags: Vec<(String, String)>) -> Self {
        self.config.global_tags = tags;
        self
    }

    /// Add a single global tag that will be appended to all metrics
    pub fn with_global_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config.global_tags.push((key.into(), value.into()));
        self
    }

    pub fn dd_logs_enabled(mut self, enabled: bool) -> Self {
        self.config.dd_logs_enabled = enabled;
        self
    }

    pub fn json_logging(mut self, enabled: bool) -> Self {
        self.config.json_logging = enabled;
        self
    }

    /// Override the sync_metrics setting. When `true`, each metric send
    /// blocks until the UDP packet is written (required for Lambda).
    /// Defaults to auto-detection via `AWS_LAMBDA_FUNCTION_NAME`.
    pub fn sync_metrics(mut self, enabled: bool) -> Self {
        self.config.sync_metrics = enabled;
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
    /// Datadog logger provider (must be kept alive and shutdown on exit)
    pub logger_provider: Option<opentelemetry_sdk::logs::SdkLoggerProvider>,
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
    let (tracer_provider, logger_provider) = if config.datadog_enabled {
        match tracing::init_datadog(&config) {
            Ok((tp, lp)) => {
                eprintln!("✅ Sideways Telemetry: Datadog tracing initialized");
                if lp.is_some() {
                    eprintln!("✅ Sideways Telemetry: Datadog log ingestion initialized");
                }
                (Some(tp), lp)
            }
            Err(err) => {
                eprintln!(
                    "⚠️  Sideways Telemetry: Datadog tracing unavailable: {}",
                    err
                );
                (None, None)
            }
        }
    } else {
        eprintln!("📊 Sideways Telemetry: Datadog tracing disabled");
        tracing::init_console_logging(&config);
        (None, None)
    };

    // Initialize metrics
    if config.metrics_enabled {
        if let Err(err) = metrics::init_metrics(&config) {
            eprintln!("⚠️  Sideways Telemetry: Metrics unavailable: {}", err);
        }
    } else {
        eprintln!("📊 Sideways Telemetry: Metrics disabled");
    }

    Telemetry {
        tracer_provider,
        logger_provider,
    }
}
