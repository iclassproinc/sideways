/// Prelude module for convenient imports
///
/// Import this to get everything you need for telemetry in one line:
///
/// ```rust
/// use pony_telemetry::prelude::*;
/// ```
///
/// This provides:
/// - All metric macros (statsd_count, statsd_gauge, statsd_time, etc.)
/// - The cadence module (required for macros to work)

// Re-export all the macros
pub use cadence_macros::{
    statsd_count, statsd_distribution, statsd_gauge, statsd_histogram, statsd_meter, statsd_set,
    statsd_time,
};

// Re-export cadence module so it's available when macros expand
// The macros internally reference `cadence::...` so this must be in scope
#[allow(unused_imports)]
pub use crate::cadence;
