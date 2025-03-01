# klogrs

A powerful command-line tool for reading and processing Kubernetes pod logs with advanced filtering, formatting, and aggregation capabilities.

## Features

- **Namespace Selection**: Specify the Kubernetes namespace to target with `-n` or `--namespace`
- **Deployment Targeting**: Focus on logs from a specific deployment with `-d` or `--deployment`
- **Follow Mode**: Stream logs in real-time with `-f` or `--follow`
- **Pattern Filtering**: Filter logs by pattern with `-g` or `--grep`
  - Multiple patterns can be combined with:
    - Comma (,) for OR logic: `-g "error,warning"` (matches either)
    - Ampersand (&) for AND logic: `-g "error&warning"` (matches both)
  - Patterns are treated as regular expressions
  - Matched keywords are highlighted by default (can be disabled with `--no-highlight`)
- **Tail Mode**: Control the number of log entries displayed per pod with `-t` or `--tail`
- **Level Filtering**: Filter logs by severity level with `-l` or `--level`
  - Supported levels: TRACE, DEBUG, INFO, WARN, ERROR, FATAL
  - Multiple levels can be combined with comma (,) for OR logic: `-l "ERROR,WARN"` (matches either)
- **Real-time Output**: Minimized buffering for immediate log display when using follow mode
- **Composite Filters**: Different filter types (grep and level) are always combined with AND logic
- **Highlighting**: Matched keywords are highlighted by default (can be disabled with `--no-highlight`)

## Usage

```bash
# Basic usage - get logs from a deployment
klogrs -n default -d nginx

# Follow logs in real-time
klogrs -n default -d nginx -f

# Filter logs containing "ERROR"
klogrs -n default -d nginx -g ERROR

# Display only the last 10 log entries per pod
klogrs -n default -d nginx -t 10

# Filter logs by minimum level (show only WARN, ERROR, FATAL)
klogrs -n default -d nginx -l WARN

# Filter logs containing either "error" OR "warning" (OR logic with comma)
klogrs -n default -d nginx -g "error,warning"

# Filter logs containing BOTH "error" AND "timeout" (AND logic with ampersand)
klogrs -n default -d nginx -g "error&timeout"

# Filter logs with either ERROR or WARN level (OR logic with comma)
klogrs -n default -d nginx -l "ERROR,WARN"

# Combining different filter types (always uses AND logic)
# Shows logs that BOTH contain "error" AND have level "WARN"
klogrs -n default -d nginx -g "error" -l "WARN"

# Disable highlighting of matched keywords
klogrs -n default -d nginx -g "error" --no-highlight
```

## Log Level Filtering

The `-l, --level` option allows filtering logs by level. This is useful for focusing on logs of a specific severity.

```bash
# Show only ERROR logs
klogrs -d nginx -l ERROR

# Show logs containing either ERROR or WARN
klogrs -d nginx -l "ERROR,WARN"
```

The level filter performs a simple case-insensitive string matching to find logs containing the specified level.
Supported levels include: TRACE, DEBUG, INFO, WARN, WARNING, ERROR, ERR, FATAL.

## Combining Filters

klogrs provides flexible ways to combine multiple filters:

1. **Within grep patterns**:
   - Use comma (,) for OR logic: `-g "error,warning"` shows logs with either "error" OR "warning"
   - Use ampersand (&) for AND logic: `-g "error&warning"` shows logs with both "error" AND "warning"
   - The legacy `--and` flag is still supported but deprecated

2. **Within level filters**:
   - Use comma (,) for OR logic: `-l "ERROR,WARN"` shows logs with either ERROR OR WARN level

3. **Between different filter types** (grep and level):
   - Always uses AND logic: `-g "error" -l "WARN"` shows logs that both contain "error" AND have "WARN" level
   - This ensures precise filtering when using both pattern and level filters

Examples:
```bash
# OR logic within grep patterns (matches logs with either "error" OR "warning")
klogrs -d nginx -g "error,warning"

# AND logic within grep patterns (matches logs with BOTH "error" AND "timeout")
klogrs -d nginx -g "error&timeout"

# OR logic within level filters (matches logs with either ERROR OR WARN level)
klogrs -d nginx -l "ERROR,WARN"

# Combining grep and level filters (always AND logic)
# Shows only logs that BOTH contain "error" AND have "WARN" level
klogrs -d nginx -g "error" -l "WARN"
```

The new separator-based approach (`&` for AND in grep patterns, `,` for OR) is more intuitive than the previous `--and` flag and provides more flexibility in constructing complex filters.

## Pattern Matching

klogrs uses regular expression matching for flexible pattern filtering:

- When filtering logs with `-g` or `--grep`, you can specify simple text patterns or more complex regular expressions.
- Multiple patterns can be combined using commas, and the tool will match logs containing any of the specified patterns (OR logic by default).
- Use the `--and-filters` flag to require logs to match all specified patterns (AND logic).

This pattern matching is particularly useful when filtering logs by specific error messages, timestamps, or other identifiers.

## Error Handling

klogrs provides a clear error handling mechanism to help users quickly identify issues:

1. **Invalid Log Levels**: When an invalid log level is specified with the `-l/--level` parameter, the program immediately terminates and displays an error message.
   For example: `klogrs -l "INVALID_LEVEL"` will return an "Invalid log level: INVALID_LEVEL" error.

2. **Invalid Regular Expressions**: When an invalid regular expression is specified with the `-g/--grep` parameter, the program immediately terminates and displays an error message.
   For example: `klogrs -g "["` will return a regular expression parsing error.

3. **Parameter Combinations**: Note that the `-l` parameter only supports comma separators for OR logic, and does not support the `&` separator.
   For example: `-l "ERROR,WARN"` is valid (matches ERROR or WARN levels), but `-l "ERROR&WARN"` will be treated as an invalid log level.

This fail-fast design helps users quickly discover and fix configuration errors, avoiding issues that might only be discovered after running for an extended period.

## Debugging

If you encounter issues while using klogrs, you can enable more detailed logging output by setting the `RUST_LOG` environment variable:

```bash
# Enable debug level logging
RUST_LOG=debug klogrs -n kube-system -d coredns

# Enable trace level logging (most detailed)
RUST_LOG=trace klogrs -n kube-system -d coredns
```

This will display more information about klogrs' internal workings, including Kubernetes API calls, filter creation, and log processing details, which can help diagnose issues.

## Installation

```bash
cargo install --path .
```

## Development

### Requirements

- Rust 2021 edition or later
- Kubernetes cluster for testing (or minikube)

### Building

```bash
cargo build
```

### Testing

```bash
cargo test
```

## License

MIT
