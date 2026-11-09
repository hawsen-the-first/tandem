//! HTTP and WebSocket server (api service).

fn main() {
    // Skeleton: replaced in a later step of the U1 code generation plan.
    #[allow(clippy::print_stdout)]
    {
        println!("tandem-api {}", env!("CARGO_PKG_VERSION"));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert!(!env!("CARGO_PKG_VERSION").is_empty());
    }
}
