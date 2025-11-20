# Sideways 🦀

> *Observability from the side - because crabs walk sideways, and so should your telemetry.*

A production-ready Rust telemetry library that provides easy-to-use Datadog tracing and StatsD metrics collection for high-performance services.

## Features

- 🎯 **Datadog Native Tracing** - OpenTelemetry-based distributed tracing via dd-trace-rs
- 📊 **StatsD Metrics** - Production-ready Cadence integration with buffering and queuing
- 🚀 **One-Line Initialization** - Simple `init_telemetry()` call sets up everything
- 🔧 **Environment-Based Config** - Configure via environment variables
- 💪 **Graceful Degradation** - Continues running even if telemetry services unavailable
- 🏷️ **Tag Support** - Full Datadog-style tag support for rich dimensional data
- 🔍 **Health Check Filtering** - Automatically filters out noisy health check spans

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
sideways = { path = "sideways" }  # Or published version
```

Initialize in your application:

```rust
use sideways::{init_telemetry, TelemetryConfig};
use sideways::prelude::*;  // Import all metrics macros
use tracing::info;

#[tokio::main]
async fn main() {
    // Load from environment variables
    let config = TelemetryConfig::from_env();
    let telemetry = init_telemetry(config).await;

    // Use tracing as normal
    info!("Application started!");

    // Emit metrics using macros - no need to import cadence!
    statsd_count!("requests.handled", 1, "status" => "success");
    statsd_time!("request.duration", 250, "endpoint" => "/api/users");
    statsd_gauge!("queue.size", 42, "queue" => "email-processing");

    // ... your application code ...

    // Cleanup on shutdown (important!)
    if let Some(tracer) = telemetry.tracer_provider {
        let _ = tracer.shutdown();
    }
}
```

## Configuration

### Environment Variables

#### Datadog Tracing

```bash
# Disable with DD_TRACE_ENABLED=false (default: enabled)
DD_TRACE_ENABLED=true

# Service configuration
DD_SERVICE=my-service
DD_ENV=production
DD_TRACE_AGENT_URL=http://localhost:8126

# Logging level
RUST_LOG=info
```

#### StatsD Metrics

```bash
# Disable with METRICS_ENABLED=false (default: enabled)
METRICS_ENABLED=true

# StatsD server
STATSD_HOST=localhost
STATSD_PORT=8125

# Metric namespace/prefix
METRICS_PREFIX=my-service
```

### Programmatic Configuration

```rust
use sideways::TelemetryConfig;

let config = TelemetryConfig::builder()
    .dd_service("my-awesome-service")
    .dd_env("production")
    .statsd_host("metrics.example.com")
    .statsd_port(8125)
    .metrics_prefix("myapp")
    .build();

let telemetry = init_telemetry(config).await;
```

## Available Metrics Macros

All metrics support Datadog-style tags:

```rust
use sideways::prelude::*;

// Counters - increment values
statsd_count!("api.requests", 1, "endpoint" => "/users", "method" => "GET");

// Gauges - arbitrary values (integers or floats)
statsd_gauge!("queue.depth", 42, "queue" => "emails");
statsd_gauge!("cpu.usage", 45.2, "host" => "web-01");

// Timers - durations in milliseconds
statsd_time!("db.query.duration", 125, "query" => "users_select");
statsd_time!("request.latency", Duration::from_millis(250), "endpoint" => "/api");

// Histograms - statistical distributions
statsd_histogram!("response.size", 1024, "endpoint" => "/api/data");
statsd_histogram!("latency", Duration::from_nanos(1500000), "service" => "auth");
statsd_histogram!("score", 98.5, "user_type" => "premium");

// Distributions - advanced histograms with percentiles
statsd_distribution!("request.bytes", 2048, "direction" => "inbound");
statsd_distribution!("memory.usage", 512.75, "process" => "worker");

// Meters - rate tracking
statsd_meter!("events.processed", 1, "event_type" => "email_sent");

// Sets - unique value counting
statsd_set!("unique.users", user_id, "platform" => "web");
```

## Usage Notes

### Importing in Your Code

Simply import the prelude to get all metrics macros:

```rust
use sideways::prelude::*;

// Now use any metric macro!
statsd_count!("requests", 1, "status" => "ok");
```

### Tracing Usage

Once telemetry is initialized, use the standard `tracing` crate for distributed tracing.

#### Basic Logging

```rust
use tracing::{info, warn, error, debug, trace};

info!("Application started");
warn!(user_id = 123, "Rate limit approaching");
error!(error = ?err, "Failed to process request");
```

#### Instrumentation

**Instrumentation is REQUIRED for distributed tracing to work properly.** The `#[instrument]` attribute automatically creates spans for functions, which are essential for:
- Request tracing across service boundaries
- Performance profiling
- Call hierarchy visualization in Datadog APM

```rust
use tracing::instrument;

// Basic instrumentation - function name becomes span name
#[instrument]
async fn process_request(id: u64) {
    info!(request_id = id, "Processing request");
    // ... do work ...
}

// Skip certain parameters (e.g., large data structures)
#[instrument(skip(data))]
async fn process_data(id: u64, data: Vec<u8>) {
    info!(size = data.len(), "Processing data");
    // ... process ...
}

// Custom span name
#[instrument(name = "db.query")]
async fn query_users(limit: i32) -> Result<Vec<User>> {
    // Query database...
}

// Add custom fields to the span
#[instrument(fields(user_type = "premium"))]
async fn process_premium_user(user_id: u64) {
    // ... processing ...
}
```

#### Structured Fields

Always use structured fields for better querying in Datadog:

```rust
// ❌ BAD - string interpolation
info!("User {} logged in from {}", user_id, ip);

// ✅ GOOD - structured fields
info!(user_id = user_id, ip = %ip, "User logged in");
```

#### Span Context

Manually create spans for more control:

```rust
use tracing::{info_span, Instrument};

async fn complex_operation() {
    // Create a span manually
    let span = info_span!("database_operation", table = "users");

    // Execute work within the span
    async {
        info!("Querying database");
        // ... db work ...
    }.instrument(span).await;
}
```

### Health Check Filtering

The library automatically filters out health check-related spans from Datadog to reduce noise:
- Spans from `tonic_health`
- Spans containing "health", "Health", or "Check"
- gRPC health check services

## Architecture

### Datadog Tracing
- Uses `datadog-opentelemetry` (dd-trace-rs) for native Datadog support
- OpenTelemetry layers with custom health check filtering
- Console logging + telemetry layers combined
- Configurable via `RUST_LOG` environment variable

### StatsD Metrics
- UDP-based for low overhead
- Buffered sink for efficient batching
- Queuing sink for asynchronous dispatch
- Global client registration for macro usage
- Automatic reconnection on failures

## Examples

### Web Service

```rust
use sideways::{init_telemetry, TelemetryConfig};
use sideways::prelude::*;
use tracing::info;

#[tokio::main]
async fn main() {
    let telemetry = init_telemetry(TelemetryConfig::from_env()).await;

    // Start your web server
    let app = create_app();

    // Track requests
    statsd_count!("server.started", 1, "version" => "1.0.0");

    serve(app).await;

    // Cleanup
    if let Some(tracer) = telemetry.tracer_provider {
        let _ = tracer.shutdown();
    }
}
```

### Worker Service

```rust
use sideways::{init_telemetry, TelemetryConfig};
use sideways::prelude::*;
use tracing::{info, instrument};

#[tokio::main]
async fn main() {
    let config = TelemetryConfig::builder()
        .dd_service("background-worker")
        .metrics_prefix("worker")
        .build();

    let telemetry = init_telemetry(config).await;

    loop {
        process_job().await;
    }
}

#[instrument]
async fn process_job() {
    statsd_count!("jobs.processed", 1, "status" => "success");
    info!("Job completed");
}
```

## Publishing

To publish this crate to crates.io:

1. Update `Cargo.toml` with repository URL and proper metadata
2. Test locally: `cargo test --all-features`
3. Publish: `cargo publish`

## License

MIT

## Credits

Built by iClassPro team, inspired by:
- [Permafrost](https://github.com/yourusername/permafrost) - gRPC patterns and Datadog setup
- [Cadence](https://github.com/56quarters/cadence) - Excellent StatsD client
- [dd-trace-rs](https://github.com/DataDog/dd-trace-rs) - Datadog native tracing
