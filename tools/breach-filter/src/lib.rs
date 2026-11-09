//! Builds the offline breached-password filter used by `tandem-credentials`.
//!
//! Input is either lines of the Have I Been Pwned corpus (`SHA1HEX[:count]`, ordered by
//! prevalence) or plain passwords, one per line. Output is the binary filter file that is embedded
//! in the application.

use std::io::BufRead;
use tandem_credentials::{BloomFilter, BloomFilterBuilder};

/// What each input line contains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    /// `SHA1HEX` optionally followed by `:count` (the Have I Been Pwned corpus).
    HashLines,
    /// One plain-text password per line.
    Plain,
}

/// How to build.
#[derive(Debug, Clone, Copy)]
pub struct BuildOptions {
    /// Line format.
    pub format: InputFormat,
    /// Use only the first this many entries (the most prevalent, for an ordered corpus).
    pub limit: Option<u64>,
    /// Target false-positive rate, for example `1e-5`.
    pub false_positive_rate: f64,
    /// For plain lists, skip passwords shorter than this many characters: the password policy
    /// already refuses them, so listing them only adds size and false positives.
    pub min_length: Option<usize>,
}

/// Why a build failed.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// Reading the input failed.
    #[error("could not read the input: {0}")]
    Read(#[from] std::io::Error),
    /// A line is not valid for the chosen format (1-based line number).
    #[error("line {line}: not a valid entry for the chosen format")]
    BadLine {
        /// The offending line.
        line: u64,
    },
    /// The input contained no entries.
    #[error("the input contained no entries")]
    Empty,
}

/// Whether an input line becomes an entry.
fn wanted(entry: &str, options: &BuildOptions) -> bool {
    if entry.trim().is_empty() {
        return false;
    }
    match (options.format, options.min_length) {
        (InputFormat::Plain, Some(minimum)) => entry.chars().count() >= minimum,
        _ => true,
    }
}

/// Count the entries the build will use, so the filter can be sized before it is filled.
///
/// # Errors
/// [`BuildError::Read`] when reading fails.
pub fn count_entries(reader: impl BufRead, options: &BuildOptions) -> Result<u64, BuildError> {
    let mut count = 0_u64;
    for line in reader.lines() {
        let line = line?;
        if !wanted(line.trim_end_matches(['\r', '\n']), options) {
            continue;
        }
        count += 1;
        if options.limit.is_some_and(|limit| count >= limit) {
            break;
        }
    }
    Ok(count)
}

/// Build a filter from the input. `expected_items` sizes the filter (use [`count_entries`]).
///
/// # Errors
/// [`BuildError`] for unreadable input, malformed lines or empty input.
pub fn build_filter(
    reader: impl BufRead,
    options: &BuildOptions,
    expected_items: u64,
) -> Result<(BloomFilter, u64), BuildError> {
    let mut builder =
        BloomFilterBuilder::with_capacity(expected_items, options.false_positive_rate);
    let mut used = 0_u64;
    for (index, line) in reader.lines().enumerate() {
        let line = line?;
        let entry = line.trim_end_matches(['\r', '\n']);
        if !wanted(entry, options) {
            continue;
        }
        match options.format {
            InputFormat::HashLines => {
                builder
                    .insert_hash_line(entry)
                    .map_err(|_| BuildError::BadLine {
                        line: index as u64 + 1,
                    })?
            }
            InputFormat::Plain => builder.insert_password(entry),
        }
        used += 1;
        if options.limit.is_some_and(|limit| used >= limit) {
            break;
        }
    }
    if used == 0 {
        return Err(BuildError::Empty);
    }
    Ok((builder.finish(), used))
}
