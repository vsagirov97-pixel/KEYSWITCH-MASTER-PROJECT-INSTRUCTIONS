//! KeySwitch core: OS-independent layout conversion.
//!
//! This crate must not perform I/O or depend on any operating system API.

/// Crate version, exposed for diagnostics.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert!(!super::VERSION.is_empty());
    }
}
