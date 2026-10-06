//! Library that carries a build script.

/// Environment captured at build time.
pub const BUILD_SCRIPT: Option<&str> = option_env!("BENCH_BUILD_SCRIPT");
