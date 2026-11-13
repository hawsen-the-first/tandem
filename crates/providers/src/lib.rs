//! Model provider adapters: the scripted mock provider and the OpenAI-compatible probe. The
//! Anthropic and Agent SDK adapters arrive in later units.

mod mock;
mod openai;

pub use mock::MockProvider;
pub use openai::OpenAiCompatibleProbe;

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
