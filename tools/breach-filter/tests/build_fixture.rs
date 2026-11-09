use std::io::Cursor;
use tandem_breach_filter::{BuildError, BuildOptions, InputFormat, build_filter, count_entries};
use tandem_credentials::BloomFilter;

fn options(format: InputFormat, limit: Option<u64>) -> BuildOptions {
    BuildOptions {
        format,
        limit,
        false_positive_rate: 1e-6,
        min_length: None,
    }
}

const HASH_LINES: &str = "\
5BAA61E4C9B93F3F0682250B6CF8331B7EE68FD8:3730471\n\
7C4A8D09CA3762AF61E59520943DC26494F8941B:1234567\n\
\n\
F7C3BC1D808E04732ADF679965CCC34CA7AE3441:99\n";

#[test]
fn br_2_3_a_hash_corpus_becomes_a_filter_that_finds_each_listed_password() {
    let expected = count_entries(
        Cursor::new(HASH_LINES),
        &options(InputFormat::HashLines, None),
    )
    .unwrap();
    assert_eq!(expected, 3);
    let (filter, used) = build_filter(
        Cursor::new(HASH_LINES),
        &options(InputFormat::HashLines, None),
        expected,
    )
    .unwrap();
    assert_eq!(used, 3);
    // SHA-1 of "password", "123456" and "123456789".
    assert!(filter.contains_password("password"));
    assert!(filter.contains_password("123456"));
    assert!(filter.contains_password("123456789"));
    assert!(!filter.contains_password("an-unlisted-passphrase-9"));
}

#[test]
fn br_2_3_the_limit_keeps_only_the_most_prevalent_entries() {
    let (filter, used) = build_filter(
        Cursor::new(HASH_LINES),
        &options(InputFormat::HashLines, Some(2)),
        2,
    )
    .unwrap();
    assert_eq!(used, 2);
    assert!(filter.contains_password("password"));
    assert!(
        !filter.contains_password("123456789"),
        "the third entry was cut off by the limit"
    );
    assert_eq!(
        count_entries(
            Cursor::new(HASH_LINES),
            &options(InputFormat::HashLines, Some(2))
        )
        .unwrap(),
        2
    );
}

#[test]
fn plain_lists_are_hashed_and_the_file_round_trips() {
    let plain = "letmein\nqwerty\r\n\nsunshine\n";
    let (filter, used) =
        build_filter(Cursor::new(plain), &options(InputFormat::Plain, None), 3).unwrap();
    assert_eq!(used, 3);
    assert!(
        filter.contains_password("qwerty"),
        "Windows line endings are handled"
    );
    let restored = BloomFilter::from_bytes(&filter.to_bytes()).unwrap();
    assert!(restored.contains_password("sunshine"));
}

#[test]
fn bad_lines_and_empty_input_are_reported() {
    let bad = "5BAA61E4C9B93F3F0682250B6CF8331B7EE68FD8\nnot-a-hash\n";
    let error =
        build_filter(Cursor::new(bad), &options(InputFormat::HashLines, None), 2).unwrap_err();
    assert!(matches!(error, BuildError::BadLine { line: 2 }));
    let empty =
        build_filter(Cursor::new("\n\n"), &options(InputFormat::Plain, None), 1).unwrap_err();
    assert!(matches!(empty, BuildError::Empty));
}

#[test]
fn the_binary_writes_a_loadable_filter_file() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("list.txt");
    let output = dir.path().join("out.bloom");
    std::fs::write(&input, "letmein\nqwerty\n").unwrap();
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_tandem-breach-filter"))
        .args([
            "--input",
            input.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
            "--format",
            "plain",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let filter = BloomFilter::from_bytes(&std::fs::read(&output).unwrap()).unwrap();
    assert!(filter.contains_password("letmein"));
    let bad = std::process::Command::new(env!("CARGO_BIN_EXE_tandem-breach-filter"))
        .status()
        .unwrap();
    assert_eq!(bad.code(), Some(2), "missing arguments exit with status 2");
}

#[test]
fn br_2_1_short_plain_passwords_can_be_left_out_because_the_policy_refuses_them_anyway() {
    let plain = "short\nlongerpassword1\nexactly10!\n";
    let mut opts = options(InputFormat::Plain, None);
    opts.min_length = Some(10);
    assert_eq!(count_entries(Cursor::new(plain), &opts).unwrap(), 2);
    let (filter, used) = build_filter(Cursor::new(plain), &opts, 2).unwrap();
    assert_eq!(used, 2);
    assert!(filter.contains_password("longerpassword1"));
    assert!(filter.contains_password("exactly10!"));
    assert!(!filter.contains_password("short"));
}
