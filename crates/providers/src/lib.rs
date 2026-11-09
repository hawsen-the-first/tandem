//! Model provider adapters. U1 contains only the scripted mock provider; the OpenAI-compatible,
//! Anthropic and Agent SDK adapters arrive in later units.

mod mock;

pub use mock::MockProvider;

/// Name of this crate, used by smoke tests and diagnostics.
pub const CRATE_NAME: &str = "tandem-providers";

#[cfg(test)]
mod tests {
    use super::CRATE_NAME;

    #[test]
    fn crate_builds() {
        assert!(CRATE_NAME.starts_with("tandem-"));
    }
}
