use crate::{TelemetryConfig, TelemetryError};
use cadence::{BufferedUdpMetricSink, QueuingMetricSink, StatsdClient};
use std::net::UdpSocket;

/// Initialize the Cadence StatsD metrics client.
///
/// This function sets up a production-grade metrics client with:
/// - UDP socket for low-overhead transmission
/// - Buffered sink for efficient batching
/// - Queuing sink for asynchronous dispatch
///
/// The client is registered globally for use with cadence-macros.
///
/// # Returns
///
/// Returns `Ok(())` if metrics are successfully initialized, or an error if:
/// - UDP socket binding fails
/// - Metric sink creation fails
///
/// # Example Usage
///
/// ```rust,no_run
/// use cadence_macros::statsd_count;
///
/// statsd_count!("some.counter", 1, "tag" => "val");
/// statsd_gauge!("some.gauge", 1, "tag" => "val");
/// statsd_gauge!("some.gauge", 1.0, "tag" => "val");
/// statsd_time!("some.timer", 1, "tag" => "val");
/// statsd_time!("some.timer", Duration::from_millis(1), "tag" => "val");
/// statsd_meter!("some.meter", 1, "tag" => "val");
/// statsd_histogram!("some.histogram", 1, "tag" => "val");
/// statsd_histogram!("some.histogram", Duration::from_nanos(1), "tag" => "val");
/// statsd_histogram!("some.histogram", 1.0, "tag" => "val");
/// statsd_distribution!("some.distribution", 1, "tag" => "val");
/// statsd_distribution!("some.distribution", 1.0, "tag" => "val");
/// statsd_set!("some.set", 1, "tag" => "val");
/// ```
pub fn init_metrics(config: &TelemetryConfig) -> Result<(), TelemetryError> {
    eprintln!(
        "📊 Initializing metrics: {}:{} with prefix '{}'",
        config.statsd_host, config.statsd_port, config.metrics_prefix
    );

    // Bind to an ephemeral UDP port
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(TelemetryError::SocketBind)?;

    // Create buffered UDP sink for efficient transmission
    let buffered =
        BufferedUdpMetricSink::from((&config.statsd_host[..], config.statsd_port), socket)
            .map_err(TelemetryError::SinkCreation)?;

    // Add queuing layer for asynchronous dispatch
    let queued = QueuingMetricSink::from(buffered);

    // Create client with namespace prefix
    let client = StatsdClient::from_sink(&config.metrics_prefix, queued);

    // Register as global default for macro usage
    cadence_macros::set_global_default(client);

    eprintln!("✅ Metrics initialized successfully");

    Ok(())
}
