use anyhow::Result;
use clap::{CommandFactory, FromArgMatches, Parser};
use std::ffi::OsString;
use tracing::debug;

/// A command-line tool for reading and processing Kubernetes pod logs
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Namespace to use
    #[arg(short = 'n', long, default_value = "default")]
    pub namespace: String,

    /// Deployment to get logs from
    #[arg(short = 'd', long)]
    pub deployment: String,

    /// Follow logs
    #[arg(long, short = 'f', default_value_t = false)]
    pub follow: bool,

    /// Grep pattern to filter logs
    /// Multiple patterns can be separated by:
    /// - comma (,) for OR logic: "error,warning" matches logs containing either "error" OR "warning"
    /// - ampersand (&) for AND logic: "error&warning" matches logs containing both "error" AND "warning"
    #[arg(long, short = 'g')]
    pub grep: Option<String>,

    /// Number of log entries to display (tail mode)
    #[arg(long, short = 't')]
    pub tail: Option<usize>,

    /// Filter logs by minimum level (TRACE, DEBUG, INFO, WARN, ERROR, FATAL)
    /// Multiple levels can be separated by comma (,) for OR logic:
    /// "ERROR,WARN" matches logs with either ERROR OR WARN level
    #[arg(long, short = 'l')]
    pub level: Option<String>,

    /// Use AND logic to combine filters within the same parameter (deprecated, use & separator instead)
    /// Note: Grep and level filters are always combined with AND logic
    #[arg(long, default_value_t = false)]
    pub and: bool,

    /// Disable highlighting of matched keywords in grep results
    #[arg(long, default_value_t = false)]
    pub no_highlight: bool,
}

/// Parse command-line arguments
pub fn parse_args<I>(args: I) -> Result<Args>
where
    I: IntoIterator<Item=OsString>,
{
    // Create command line parser
    let mut app = Args::command();

    // Get argument iterator
    let args_iter = args.into_iter();

    // Use clap's parsing method
    let matches = match app.try_get_matches_from_mut(args_iter) {
        Ok(matches) => matches,
        Err(err) => {
            // Handle special cases for --help and --version
            if let clap::error::ErrorKind::DisplayHelp = err.kind() {
                err.print().unwrap();
                std::process::exit(0);
            } else if let clap::error::ErrorKind::DisplayVersion = err.kind() {
                println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            return Err(err.into());
        }
    };

    // Build Args from matches
    let args = Args::from_arg_matches(&matches)?;

    // Debug log the arguments
    debug!("Parsed arguments: {:?}", args);

    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_args() {
        let args = parse_args(vec![
            OsString::from("klogrs"),
            OsString::from("-n"),
            OsString::from("default"),
            OsString::from("-d"),
            OsString::from("nginx"),
            OsString::from("--grep"),
            OsString::from("error"),
            OsString::from("--tail"),
            OsString::from("10"),
            OsString::from("--level"),
            OsString::from("INFO"),
        ])
            .unwrap();

        assert_eq!(args.namespace, "default");
        assert_eq!(args.deployment, "nginx");
        assert_eq!(args.grep, Some("error".to_string()));
        assert_eq!(args.follow, false);
        assert_eq!(args.tail, Some(10));
        assert_eq!(args.level, Some("INFO".to_string()));
        assert_eq!(args.and, false);
        assert_eq!(args.no_highlight, false);
    }

    #[test]
    fn test_parse_args_with_and_filters() {
        let args = parse_args(vec![
            OsString::from("klogrs"),
            OsString::from("--namespace"),
            OsString::from("default"),
            OsString::from("--deployment"),
            OsString::from("nginx"),
            OsString::from("--grep"),
            OsString::from("error,warning"),
            OsString::from("--level"),
            OsString::from("INFO"),
            OsString::from("--and"),
        ])
            .unwrap();

        assert_eq!(args.namespace, "default");
        assert_eq!(args.deployment, "nginx");
        assert_eq!(args.grep, Some("error,warning".to_string()));
        assert_eq!(args.level, Some("INFO".to_string()));
        assert_eq!(args.and, true);
        assert_eq!(args.no_highlight, false);
    }
}
