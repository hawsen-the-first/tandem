//! Command line for the breached-password filter builder.
//!
//! ```text
//! tandem-breach-filter --input <file> --output <file> [--format hash|plain] [--limit N]
//!                      [--min-length N] [--fpr 1e-5]
//! ```

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::process::ExitCode;
use tandem_breach_filter::{BuildOptions, InputFormat, build_filter, count_entries};

const USAGE: &str = "usage: tandem-breach-filter --input <file> --output <file> \
[--format hash|plain] [--limit N] [--min-length N] [--fpr 1e-5]\n\
  --format hash    lines of SHA1HEX[:count], most prevalent first (the Have I Been Pwned corpus; default)\n\
  --format plain   one plain-text password per line\n\
  --limit N        use only the first N entries\n\
  --min-length N   (plain lists) skip passwords shorter than N characters\n\
  --fpr            target false-positive rate (default 1e-5)";

struct Arguments {
    input: PathBuf,
    output: PathBuf,
    options: BuildOptions,
}

fn parse_arguments(mut args: impl Iterator<Item = String>) -> Result<Arguments, String> {
    let (mut input, mut output) = (None, None);
    let mut options = BuildOptions {
        format: InputFormat::HashLines,
        limit: None,
        false_positive_rate: 1e-5,
        min_length: None,
    };
    while let Some(flag) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match flag.as_str() {
            "--input" => input = Some(PathBuf::from(value("--input")?)),
            "--output" => output = Some(PathBuf::from(value("--output")?)),
            "--format" => {
                options.format = match value("--format")?.as_str() {
                    "hash" => InputFormat::HashLines,
                    "plain" => InputFormat::Plain,
                    other => return Err(format!("unknown format `{other}`")),
                }
            }
            "--limit" => {
                options.limit = Some(
                    value("--limit")?
                        .parse()
                        .map_err(|_| "--limit must be a whole number".to_owned())?,
                );
            }
            "--min-length" => {
                options.min_length = Some(
                    value("--min-length")?
                        .parse()
                        .map_err(|_| "--min-length must be a whole number".to_owned())?,
                );
            }
            "--fpr" => {
                options.false_positive_rate = value("--fpr")?
                    .parse()
                    .map_err(|_| "--fpr must be a number such as 1e-5".to_owned())?;
            }
            "--help" | "-h" => return Err(String::new()),
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(Arguments {
        input: input.ok_or("--input is required")?,
        output: output.ok_or("--output is required")?,
        options,
    })
}

fn run(arguments: &Arguments) -> Result<String, Box<dyn std::error::Error>> {
    let expected = count_entries(
        BufReader::new(File::open(&arguments.input)?),
        &arguments.options,
    )?;
    let (filter, used) = build_filter(
        BufReader::new(File::open(&arguments.input)?),
        &arguments.options,
        expected,
    )?;
    std::fs::write(&arguments.output, filter.to_bytes())?;
    Ok(format!(
        "wrote {} ({} entries, {} bytes)",
        arguments.output.display(),
        used,
        filter.byte_len()
    ))
}

fn main() -> ExitCode {
    match parse_arguments(std::env::args().skip(1)) {
        Err(message) => {
            #[allow(clippy::print_stderr)]
            {
                if !message.is_empty() {
                    eprintln!("error: {message}");
                }
                eprintln!("{USAGE}");
            }
            ExitCode::from(2)
        }
        Ok(arguments) => match run(&arguments) {
            Ok(summary) => {
                #[allow(clippy::print_stdout)]
                {
                    println!("{summary}");
                }
                ExitCode::SUCCESS
            }
            Err(error) => {
                #[allow(clippy::print_stderr)]
                {
                    eprintln!("error: {error}");
                }
                ExitCode::FAILURE
            }
        },
    }
}
