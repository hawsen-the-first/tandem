use crate::trace::TraceContext;
use tandem_support::FakeRng;

#[test]
fn a_root_round_trips_through_the_header() {
    let rng = FakeRng::new(7);
    let root = TraceContext::new_root(&rng).unwrap();
    let header = root.to_header();
    assert_eq!(header.len(), 55);
    assert!(header.starts_with("00-"));
    assert!(header.ends_with("-01"));
    assert_eq!(TraceContext::parse(&header), Some(root));
    assert_eq!(root.trace_id().len(), 32);
    assert_eq!(root.span_id().len(), 16);
}

#[test]
fn a_child_keeps_the_trace_and_changes_the_span() {
    let rng = FakeRng::new(8);
    let root = TraceContext::new_root(&rng).unwrap();
    let child = root.child(&rng).unwrap();
    assert_eq!(child.trace_id(), root.trace_id());
    assert_ne!(child.span_id(), root.span_id());
}

#[test]
fn the_rfc_example_parses() {
    let parsed =
        TraceContext::parse("00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01").unwrap();
    assert_eq!(parsed.trace_id(), "0af7651916cd43dd8448eb211c80319c");
    assert_eq!(parsed.span_id(), "b7ad6b7169203331");
}

#[test]
fn malformed_headers_are_refused() {
    for header in [
        "",
        "garbage",
        "01-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
        "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331",
        "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01-extra",
        "00-00000000000000000000000000000000-b7ad6b7169203331-01",
        "00-0af7651916cd43dd8448eb211c80319c-0000000000000000-01",
        "00-0AF7651916CD43DD8448EB211C80319C-b7ad6b7169203331-01",
        "00-0af7651916cd43dd8448eb211c80319-b7ad6b7169203331-01",
        "00-zzf7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
    ] {
        assert_eq!(TraceContext::parse(header), None, "{header:?}");
    }
}

#[test]
fn the_sampled_flag_is_preserved() {
    let parsed =
        TraceContext::parse("00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-00").unwrap();
    assert!(parsed.to_header().ends_with("-00"));
}
