# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Sideways** is a production-ready Rust telemetry library that provides Datadog tracing and StatsD metrics collection for high-performance services. The name comes from the observation that "crabs walk sideways, and so should your telemetry."

## Build and Development Commands

### Building
```bash
cargo build
cargo build --release
```

### Testing
```bash
cargo test
cargo test --all-features
```

### Publishing
Before publishing to crates.io:
```bash
cargo test --all-features
cargo publish --dry-run  # Test the publishing process
cargo publish            # Actually publish
```

## Architecture

### Module Structure

The library is organized into three core modules:

1. **`src/lib.rs`** - Main entry point with configuration and initialization
   - `TelemetryConfig` - Configuration struct with builder pattern and environment-based loading
   - `init_telemetry()` - Single initialization function that sets up both tracing and metrics
   - `Telemetry` struct - Return value that holds tracer provider (must be kept alive and shutdown on exit)

2. **`src/tracing.rs`** - Datadog tracing integration
   - Uses `datadog-opentelemetry` (dd-trace-rs) for native Datadog support
   - OpenTelemetry-based tracing layers with custom filtering
   - `HealthCheckFilter` - Custom filter to exclude health check spans (tonic_health, grpc.health, etc.)
   - Supports both full Datadog tracing and console-only logging fallback

3. **`src/metrics.rs`** - StatsD metrics client
   - Uses Cadence for StatsD integration
   - Production setup: UDP socket → BufferedUdpMetricSink → QueuingMetricSink → StatsdClient
   - Registers global client for use with cadence-macros
   - Supports all StatsD metric types: counters, gauges, timers, histograms, distributions, meters, sets

4. **`src/prelude.rs`** - Convenience module
   - Re-exports all metric macros from cadence-macros
   - Re-exports the cadence module (required for macro expansion)
   - Allows users to import everything with `use sideways::prelude::*;`

### Key Design Patterns

**One-Line Initialization**: The library focuses on simplicity with `init_telemetry()` setting up everything needed for both tracing and metrics.

**Graceful Degradation**: If Datadog or StatsD services are unavailable, the library continues running without crashing the application. Errors are logged to stderr but not propagated.

**Environment-Based Configuration**: Primary configuration method is via environment variables (DD_TRACE_ENABLED, DD_SERVICE, DD_ENV, STATSD_HOST, etc.) with programmatic override via builder pattern.

**Global State for Metrics**: The StatsD client is registered globally using `cadence_macros::set_global_default()` to enable convenient macro usage without passing clients around.

### Tracing Architecture

The tracing layer uses a layered subscriber approach:
- Base: `Registry::default()` subscriber
- Console layer: Standard formatted logging (no ANSI colors)
- Telemetry layer: OpenTelemetry → Datadog with health check filtering
- Both layers use the same `EnvFilter` for log level control

**Important**: The `HealthCheckFilter` excludes spans from tonic_health and any span names/targets containing "health", "Health", or "Check" to reduce noise in Datadog APM.

### Metrics Architecture

StatsD metrics flow through multiple sinks for efficiency:
1. **UDP Socket** - Bound to ephemeral port (0.0.0.0:0)
2. **BufferedUdpMetricSink** - Batches metrics before sending
3. **QueuingMetricSink** - Asynchronous dispatch to avoid blocking
4. **StatsdClient** - Client with namespace/prefix support

## Important Implementation Details

### Tracer Provider Lifecycle
The `tracer_provider` returned by `init_telemetry()` **must** be:
- Kept alive for the application lifetime
- Properly shutdown on application exit: `tracer_provider.shutdown()`

Failure to shutdown properly can result in lost traces.

### Instrumentation Requirement
For distributed tracing to work properly, functions must use the `#[instrument]` attribute from the `tracing` crate. Without instrumentation, spans won't be created and request tracing across service boundaries won't work.

### Macro Usage Pattern
All metrics macros support Datadog-style tags:
```rust
statsd_count!("metric.name", value, "tag_key" => "tag_value", "key2" => "value2");
```

The prelude module must be imported for macros to work correctly because the macros reference `cadence::...` internally.

### Dependencies
- **dd-trace-rs**: Cloned from git (main branch) - not yet published to crates.io
- **Cadence**: Production-ready StatsD client (1.6+)
- **OpenTelemetry**: Version 0.31.0 with tracing-opentelemetry 0.32.0

## Configuration

### Environment Variables
- `DD_TRACE_ENABLED` - Enable/disable Datadog (default: true)
- `DD_SERVICE` - Service name for Datadog
- `DD_ENV` - Environment name (e.g., production, staging)
- `DD_TRACE_AGENT_URL` - Agent URL (default: http://localhost:8126)
- `RUST_LOG` - Log level filter (default: info)
- `METRICS_ENABLED` - Enable/disable metrics (default: true)
- `STATSD_HOST` - StatsD server host (default: localhost)
- `STATSD_PORT` - StatsD server port (default: 8125)
- `METRICS_PREFIX` - Metric namespace prefix (default: sideways)

### Default Values
The library uses "sideways-service" and "sideways" as default service name and metrics prefix. These should typically be overridden via environment variables or builder pattern to match your actual service name.
