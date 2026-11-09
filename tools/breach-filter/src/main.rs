//! Builds the offline breached-password filter from a password-hash list.

fn main() {
    // Skeleton: replaced in a later step of the U1 code generation plan.
    #[allow(clippy::print_stdout)]
    {
        println!("tandem-breach-filter {}", env!("CARGO_PKG_VERSION"));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert!(!env!("CARGO_PKG_VERSION").is_empty());
    }
}
