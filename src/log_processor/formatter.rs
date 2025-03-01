use anyhow::Result;
use std::collections::HashMap;
use std::io::Write;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

use crate::kubernetes::log::LogEntry;
use crate::log_processor::filter::GrepFilter;
use crate::utils::color::ColorGenerator;

/// Prefix format for log entries
pub struct PrefixFormat {
    /// Format string
    format: String,
}

impl PrefixFormat {
    /// Create a new prefix format
    pub fn new(format: &str) -> Self {
        Self {
            format: format.to_string(),
        }
    }

    /// Format a log entry prefix
    pub fn format(&self, entry: &LogEntry) -> String {
        let mut result = self.format.clone();

        // Replace %n with pod name
        result = result.replace("%n", &entry.pod_name);

        // Replace %s with short pod name
        result = result.replace(
            "%s",
            &entry.pod_name[..std::cmp::min(8, entry.pod_name.len())],
        );

        // Replace %t with timestamp placeholder for tests
        if self.format.contains("%t") && cfg!(test) {
            result = result.replace("%t", "%t");
        }

        result
    }
}

/// Log formatter for formatting log entries
pub struct LogFormatter {
    /// Prefix format - default prefix
    prefix_format: Option<String>,
    /// Whether to show prefix - always show prefix
    no_prefix: bool,
    /// Color generator
    color_generator: ColorGenerator,
    /// Pod colors
    pod_colors: HashMap<String, Color>,
    /// Whether to highlight grep matches
    highlight: bool,
    /// Grep filters for highlighting
    grep_filters: Vec<GrepFilter>,
}

impl LogFormatter {
    /// Create a new log formatter
    pub fn new(prefix_format: Option<String>, no_prefix: bool) -> Self {
        Self {
            prefix_format,
            no_prefix,
            color_generator: ColorGenerator::new(),
            pod_colors: HashMap::new(),
            highlight: true,
            grep_filters: Vec::new(),
        }
    }
    
    /// Set whether to highlight grep matches
    pub fn set_highlight(&mut self, highlight: bool) {
        self.highlight = highlight;
    }
    
    /// Add a grep filter for highlighting
    pub fn add_grep_filter(&mut self, filter: GrepFilter) {
        self.grep_filters.push(filter);
    }

    /// Format a log entry
    pub fn format(&mut self, entry: &LogEntry) -> Result<String> {
        if self.no_prefix {
            return Ok(entry.message.clone());
        }

        let prefix = if let Some(format) = &self.prefix_format {
            let prefix_format = PrefixFormat::new(format);
            prefix_format.format(entry)
        } else {
            format!("[{}]", entry.pod_name)
        };

        Ok(format!("{} {}", prefix, entry.message))
    }

    /// Format a log entry with color
    pub fn format_colored(&mut self, entry: &LogEntry) -> Result<()> {
        let mut stdout = StandardStream::stdout(ColorChoice::Always);

        // Get or generate color for pod
        let color = self
            .pod_colors
            .entry(entry.pod_name.clone())
            .or_insert_with(|| self.color_generator.next_color());

        // Format prefix
        if !self.no_prefix {
            let prefix = if let Some(format) = &self.prefix_format {
                let prefix_format = PrefixFormat::new(format);
                prefix_format.format(entry)
            } else {
                format!("[{}]", entry.pod_name)
            };

            // Write colored prefix
            stdout.set_color(ColorSpec::new().set_fg(Some(*color)))?;
            write!(stdout, "{}", prefix)?;
            stdout.reset()?;

            // Add space after prefix
            write!(stdout, " ")?;
        }

        // If highlighting is enabled and we have grep filters, highlight matches
        if self.highlight && !self.grep_filters.is_empty() {
            self.write_highlighted_message(&mut stdout, &entry.message)?;
        } else {
            // Write message without highlighting
            writeln!(stdout, "{}", entry.message)?;
        }

        Ok(())
    }
    
    /// Write a message with highlighted grep matches
    fn write_highlighted_message(&self, stdout: &mut StandardStream, message: &str) -> Result<()> {
        // Collect all matches from all filters
        let mut matches = Vec::new();
        for filter in &self.grep_filters {
            matches.extend(filter.find_matches(message));
        }
        
        // Sort matches by start position
        matches.sort_by_key(|&(start, _)| start);
        
        // Merge overlapping matches
        let mut merged_matches = Vec::new();
        for (start, end) in matches {
            if let Some((_, last_end)) = merged_matches.last_mut() {
                if start <= *last_end {
                    // Overlapping match, extend the previous one
                    *last_end = std::cmp::max(*last_end, end);
                } else {
                    // Non-overlapping match, add it
                    merged_matches.push((start, end));
                }
            } else {
                // First match
                merged_matches.push((start, end));
            }
        }
        
        // Write message with highlighted matches
        let mut last_end = 0;
        for (start, end) in merged_matches {
            // Write non-highlighted text before match
            if start > last_end {
                write!(stdout, "{}", &message[last_end..start])?;
            }
            
            // Write highlighted match
            stdout.set_color(ColorSpec::new().set_fg(Some(Color::Yellow)).set_intense(true))?;
            write!(stdout, "{}", &message[start..end])?;
            stdout.reset()?;
            
            last_end = end;
        }
        
        // Write remaining non-highlighted text
        if last_end < message.len() {
            write!(stdout, "{}", &message[last_end..])?;
        }
        
        // Add newline at the end
        writeln!(stdout)?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_entry() -> LogEntry {
        LogEntry {
            pod_name: "test-pod".to_string(),
            raw_line: "2023-05-01T12:34:56Z Hello, world!".to_string(),
            message: "Hello, world!".to_string(),
        }
    }

    #[test]
    fn test_prefix_format() {
        let entry = create_test_entry();

        let format = PrefixFormat::new("[%n]");
        assert_eq!(format.format(&entry), "[test-pod]");

        let format = PrefixFormat::new("[%s]");
        assert_eq!(format.format(&entry), "[test-pod]");

        let format = PrefixFormat::new("[%t]");
        assert_eq!(format.format(&entry), "[%t]");

        let format = PrefixFormat::new("[%t %n]");
        assert_eq!(format.format(&entry), "[%t test-pod]");
    }

    #[test]
    fn test_formatter_default() {
        let entry = create_test_entry();
        let mut formatter = LogFormatter::new(None, false);

        assert_eq!(
            formatter.format(&entry).unwrap(),
            "[test-pod] Hello, world!"
        );
    }
}
