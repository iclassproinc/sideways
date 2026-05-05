use crate::{TelemetryConfig, TelemetryError};
use cadence::{BufferedUdpMetricSink, QueuingMetricSink, StatsdClient, UdpMetricSink};
use std::net::UdpSocket;

/// Initialize the Cadence StatsD metrics client.
///
/// Two dispatch modes are supported, selected via `TelemetryConfig::sync_metrics`
/// (auto-enabled when `AWS_LAMBDA_FUNCTION_NAME` is set):
///
/// **Sync mode (Lambda):** `UdpMetricSink` — each `statsd_*!` call sends a
/// UDP packet before returning. Lambda freezes the execution environment the
/// moment the handler returns, which would leave the async queuing thread
/// suspended before it can flush, causing metrics to be lost or deferred.
///
/// **Async mode (ECS / long-running services):** `BufferedUdpMetricSink` +
/// `QueuingMetricSink` — metrics are batched and dispatched on a background
/// thread for higher throughput.
pub fn init_metrics(config: &TelemetryConfig) -> Result<(), TelemetryError> {
    let tags_info = if config.global_tags.is_empty() {
        String::new()
    } else {
        format!(
            " with global tags: [{}]",
            config
                .global_tags
                .iter()
                .map(|(k, v)| format!("{}:{}", k, v))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    let mode = if config.sync_metrics { "sync" } else { "async" };
    eprintln!(
        "📊 Initializing metrics ({}): {}:{} prefix '{}'{}",
        mode, config.statsd_host, config.statsd_port, config.metrics_prefix, tags_info
    );

    if config.sync_metrics {
        // Synchronous: each metric send blocks until the UDP packet is written.
        // Required for Lambda where the execution environment is frozen after
        // the handler returns, before the async queue thread can flush.
        let socket = UdpSocket::bind("0.0.0.0:0").map_err(TelemetryError::SocketBind)?;
        let sink = UdpMetricSink::from((&config.statsd_host[..], config.statsd_port), socket)
            .map_err(TelemetryError::SinkCreation)?;
        let mut builder = StatsdClient::builder(&config.metrics_prefix, sink);
        for (key, value) in &config.global_tags {
            builder = builder.with_tag(key, value);
        }
        cadence_macros::set_global_default(builder.build());
    } else {
        // Asynchronous: metrics are buffered and dispatched on a background
        // thread for efficient batching in long-running services.
        let socket = UdpSocket::bind("0.0.0.0:0").map_err(TelemetryError::SocketBind)?;
        let buffered =
            BufferedUdpMetricSink::from((&config.statsd_host[..], config.statsd_port), socket)
                .map_err(TelemetryError::SinkCreation)?;
        let queued = QueuingMetricSink::from(buffered);
        let mut builder = StatsdClient::builder(&config.metrics_prefix, queued);
        for (key, value) in &config.global_tags {
            builder = builder.with_tag(key, value);
        }
        cadence_macros::set_global_default(builder.build());
    }

    eprintln!("✅ Metrics initialized");
    Ok(())
}
