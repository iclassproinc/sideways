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

    if config.json_logging {
        let console_layer = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_timer(tracing_subscriber::fmt::time::UtcTime::rfc_3339())
            .json()
            .flatten_event(true)
            .with_target(true)
            .with_span_list(true)
            .with_filter(env_filter);

        match tracing::subscriber::set_global_default(subscriber.with(console_layer)) {
            Ok(_) => eprintln!("✅ Console logging initialized (JSON)"),
            Err(e) => eprintln!("❌ Failed to initialize console logging: {}", e),
        }
    } else {
        let console_layer = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_filter(env_filter);

        match tracing::subscriber::set_global_default(subscriber.with(console_layer)) {
            Ok(_) => eprintln!("✅ Console logging initialized"),
            Err(e) => eprintln!("❌ Failed to initialize console logging: {}", e),
        }
    }
}

/// Custom filter to exclude health check spans from tracing
struct HealthCheckFilter;

impl<S> Filter<S> for HealthCheckFilter
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn enabled(&self, meta: &Metadata<'_>, _cx: &LayerContext<'_, S>) -> bool {
        let target = meta.target();

        if target.starts_with("tonic_health") {
            return false;
        }

        if target.contains("grpc.health") || target.contains("Health") {
            return false;
        }

        let name = meta.name();
        if name.contains("health") || name.contains("Health") || name.contains("Check") {
            return false;
        }

        true
    }
}

/// Initialize Datadog tracing and optionally Datadog log ingestion.
///
/// Returns Ok with (tracer_provider, optional logger_provider) if Datadog is available,
/// or Err if initialization fails.
pub fn init_datadog(
    config: &TelemetryConfig,
) -> Result<
    (
        opentelemetry_sdk::trace::SdkTracerProvider,
        Option<opentelemetry_sdk::logs::SdkLoggerProvider>,
    ),
    TelemetryError,
> {
    let dd_config = datadog_opentelemetry::configuration::Config::builder()
        .set_service(config.dd_service.clone())
        .set_agent_host(config.dd_trace_agent_url.clone().into())
        .build();

    let tracer_provider = datadog_opentelemetry::tracing()
        .with_config(dd_config.clone())
        .init();

    // Optionally initialize Datadog log ingestion
    let logger_provider = if config.dd_logs_enabled {
        match std::panic::catch_unwind(|| {
            datadog_opentelemetry::logs()
                .with_config(dd_config)
                .init()
        }) {
            Ok(provider) => Some(provider),
            Err(_) => {
                eprintln!("⚠️  Datadog log ingestion initialization failed, continuing without it");
                None
            }
        }
    } else {
        None
    };

    let tracer = tracer_provider.tracer(config.dd_service.clone());

    // Use a macro to avoid repeating the subscriber assembly for each combination
    // of json/non-json and with/without logs layer
    macro_rules! set_subscriber {
        ($console_layer:expr) => {{
            let env_filter = get_env_filter(config);
            let subscriber = Registry::default();
            let console = $console_layer.with_filter(env_filter);
            let telemetry = tracing_opentelemetry::layer()
                .with_tracer(tracer)
                .with_filter(HealthCheckFilter)
                .with_filter(tracing_subscriber::filter::LevelFilter::INFO);

            if let Some(ref lp) = logger_provider {
                let logs_filter = get_env_filter(config);
                let logs_layer =
                    opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(lp)
                        .with_filter(logs_filter);
                tracing::subscriber::set_global_default(
                    subscriber.with(console).with(telemetry).with(logs_layer),
                )
                .map_err(|e| TelemetryError::SubscriberInit(e.to_string()))?;
            } else {
                tracing::subscriber::set_global_default(
                    subscriber.with(console).with(telemetry),
                )
                .map_err(|e| TelemetryError::SubscriberInit(e.to_string()))?;
            }
        }};
    }

    if config.json_logging {
        set_subscriber!(tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_timer(tracing_subscriber::fmt::time::UtcTime::rfc_3339())
            .json()
            .flatten_event(true)
            .with_target(true)
            .with_span_list(true));
    } else {
        set_subscriber!(tracing_subscriber::fmt::layer().with_ansi(false));
    }

    tracing::info!("🦀 Datadog tracing initialized successfully");

    Ok((tracer_provider, logger_provider))
}
