//! Environment configuration read by the compiler's tuning and diagnostic switches.
//!
//! Every TSRS_* switch (and RAYON_NUM_THREADS for the compiler's own pool) is read through this module.
//! With the embedded feature, an embedding host's process environment cannot change compiler
//! behavior: these reads report "not present", so every switch takes its built-in default and the host
//! configures the compiler through its API only.

use std::env::VarError;
use std::ffi::OsString;

/// Whether this build ignores the process environment for compiler switches.
pub const IGNORES_ENVIRONMENT: bool = cfg!(feature = "embedded");

fn governed(name: &str) -> bool {
    name.starts_with("TSRS_") || name == "RAYON_NUM_THREADS"
}

/// std::env::var for compiler switches.
pub fn var(name: &str) -> Result<String, VarError> {
    if IGNORES_ENVIRONMENT && governed(name) {
        return Err(VarError::NotPresent);
    }
    std::env::var(name)
}

/// std::env::var_os for compiler switches.
pub fn var_os(name: &str) -> Option<OsString> {
    if IGNORES_ENVIRONMENT && governed(name) {
        return None;
    }
    std::env::var_os(name)
}

