use crate::{TelemetryConfig, TelemetryError};
use opentelemetry::trace::TracerProvider;
use tracing::Metadata;
use tracing_subscriber::layer::{Context as LayerContext, Filter, Layer, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{EnvFilter, Registry};

/// Get an EnvFilter from configuration.
fn get_env_filter(config: &TelemetryConfig) -> EnvFilter {
    eprintln!("📊 Configuring EnvFilter with RUST_LOG: {}", config.rust_log);

    EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&config.rust_log))
        .unwrap_or_else(|e| {
            eprintln!(
                "⚠️  Failed to parse RUST_LOG filter: {}. Using default 'info'",
                e
            );
            EnvFilter::new("info")
        })
}

/// Initialize console-only logging without Datadog telemetry.
pub fn init_console_logging(config: &TelemetryConfig) {
    let env_filter = get_env_filter(config);

    let subscriber = Registry::default();
    let console_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_filter(env_filter);

    let layered_subscriber = subscriber.with(console_layer);

    match tracing::subscriber::set_global_default(layered_subscriber) {
        Ok(_) => eprintln!("✅ Console logging initialized"),
        Err(e) => eprintln!("❌ Failed to initialize console logging: {}", e),
    }
}

/// Custom filter to exclude health check spans from tracing
struct HealthCheckFilter;

impl<S> Filter<S> for HealthCheckFilter
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn enabled(&self, meta: &Metadata<'_>, _cx: &LayerContext<'_, S>) -> bool {
        // Filter out spans from health check endpoints
        let target = meta.target();

        // Exclude tonic health check service
        if target.starts_with("tonic_health") {
            return false;
        }

        // Exclude grpc.health.v1.Health service
        if target.contains("grpc.health") || target.contains("Health") {
            return false;
        }

        // Check span name for health check patterns
        let name = meta.name();
        if name.contains("health") || name.contains("Health") || name.contains("Check") {
            return false;
        }

        true
    }
}

/// Initialize Datadog tracing using dd-trace-rs.
///
/// Returns Ok with provider if Datadog is available, or Err if initialization fails.
pub fn init_datadog(
    config: &TelemetryConfig,
) -> Result<opentelemetry_sdk::trace::SdkTracerProvider, TelemetryError> {
    let env_filter = get_env_filter(config);

    let subscriber = Registry::default();

    let tracer_provider = datadog_opentelemetry::tracing()
        .with_config(
            datadog_opentelemetry::core::Config::builder()
                .set_service(config.dd_service.clone())
                .set_agent_host(config.dd_trace_agent_url.clone().into())
                .build(),
        )
        .init();

    let console_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_filter(env_filter);

    let telemetry_layer = tracing_opentelemetry::layer()
        .with_tracer(tracer_provider.tracer(config.dd_service.clone()))
        .with_filter(HealthCheckFilter)
        .with_filter(tracing_subscriber::filter::LevelFilter::INFO);

    let layered_subscriber = subscriber.with(console_layer).with(telemetry_layer);

    tracing::subscriber::set_global_default(layered_subscriber)
        .map_err(|e| TelemetryError::SubscriberInit(e.to_string()))?;

    tracing::info!("🦀 Datadog tracing initialized successfully");

    Ok(tracer_provider)
}
