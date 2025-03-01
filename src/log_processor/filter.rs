use anyhow::Result;
use regex::Regex;

use crate::kubernetes::log::LogEntry;

/// Trait for log filters
pub trait Filter: Send + Sync {
    /// Apply the filter to a log entry
    fn apply(&self, entry: &LogEntry) -> bool;

    /// Get the filter description
    fn description(&self) -> String;
}

/// Filter logs using a regular expression
#[derive(Clone)]
pub struct GrepFilter {
    pattern: String,
    regex: Regex,
}

impl GrepFilter {
    /// Create a new grep filter
    pub fn new(pattern: &str) -> Result<Self> {
        // Try to create a regular expression, if it fails, use the escaped string
        let regex = match Regex::new(pattern) {
            Ok(re) => re,
            Err(_) => {
                // If regex creation fails, use the escaped string for exact matching
                let escaped = regex::escape(pattern);
                Regex::new(&escaped)?
            }
        };
        
        Ok(Self {
            pattern: pattern.to_string(),
            regex,
        })
    }
    
    /// Get the pattern string
    pub fn pattern(&self) -> &str {
        &self.pattern
    }
    
    /// Find all matches in a string
    pub fn find_matches(&self, text: &str) -> Vec<(usize, usize)> {
        self.regex.find_iter(text)
            .map(|m| (m.start(), m.end()))
            .collect()
    }
}

impl Filter for GrepFilter {
    fn apply(&self, entry: &LogEntry) -> bool {
        self.regex.is_match(&entry.raw_line)
    }

    fn description(&self) -> String {
        format!("grep(\"{}\")", self.pattern)
    }
}

/// Filter logs by log level
pub struct LevelFilter {
    level_str: String,
    regex: Regex,
}

impl LevelFilter {
    /// Create a new level filter
    pub fn new(level: &str) -> Result<Self> {
        // Validate if the log level is valid
        let valid_levels = ["TRACE", "DEBUG", "INFO", "WARN", "WARNING", "ERROR", "ERR", "FATAL", "CRITICAL"];
        let upper_level = level.to_uppercase();
        
        if !valid_levels.contains(&upper_level.as_str()) {
            // Return error directly without logging, as the caller will handle it
            return Err(anyhow::anyhow!("Invalid log level: {}", level));
        }
        
        // Create a more precise regular expression to match common log level formats
        let pattern = if upper_level == "ERROR" {
            r"(?i)(\[ERROR\])|(\bERROR:)|(\d{4}[-/]\d{2}[-/]\d{2}.*?\bERROR:)|(\bERROR\b)".to_string()
        } else if upper_level == "WARN" || upper_level == "WARNING" {
            r"(?i)(\[WARN(?:ING)?\])|(\bWARN(?:ING)?:)|(\d{4}[-/]\d{2}[-/]\d{2}.*?\bWARN(?:ING)?:)|(\bWARN(?:ING)?\b)".to_string()
        } else {
            // For other levels, use a generic pattern
            format!(
                r"(?i)(\[{0}\])|(\b{0}:)|(\d{{4}}[-/]\d{{2}}[-/]\d{{2}}.*?\b{0}:)|(\b{0}\b)",
                upper_level
            )
        };
        
        let regex = Regex::new(&pattern)?;
        
        Ok(Self { 
            level_str: upper_level,
            regex,
        })
    }
    
    /// Special handling for certain log lines to exclude false positives
    fn should_exclude(&self, text: &str) -> bool {
        // Exclude cases where Trace[number]: contains "error"
        if self.level_str == "ERROR" && text.contains("Trace[") && text.contains("error:") {
            return true;
        }
        false
    }
}

impl Filter for LevelFilter {
    fn apply(&self, entry: &LogEntry) -> bool {
        // First check if this log should be excluded
        if self.should_exclude(&entry.raw_line) {
            return false;
        }
        
        // Use regex for more precise matching
        self.regex.is_match(&entry.raw_line)
    }

    fn description(&self) -> String {
        format!("level({})", self.level_str)
    }
}

/// Composite filter that combines multiple filters with AND logic
pub struct AndFilter {
    filters: Vec<Box<dyn Filter>>,
}

impl AndFilter {
    /// Create a new AND filter
    pub fn new(filters: Vec<Box<dyn Filter>>) -> Self {
        Self { filters }
    }
}

impl Filter for AndFilter {
    fn apply(&self, entry: &LogEntry) -> bool {
        self.filters.iter().all(|f| f.apply(entry))
    }

    fn description(&self) -> String {
        let descriptions: Vec<String> = self.filters.iter().map(|f| f.description()).collect();
        format!("({})", descriptions.join(" AND "))
    }
}

/// Composite filter that combines multiple filters with OR logic
pub struct OrFilter {
    filters: Vec<Box<dyn Filter>>,
}

impl OrFilter {
    /// Create a new OR filter
    pub fn new(filters: Vec<Box<dyn Filter>>) -> Self {
        Self { filters }
    }
}

impl Filter for OrFilter {
    fn apply(&self, entry: &LogEntry) -> bool {
        self.filters.iter().any(|f| f.apply(entry))
    }

    fn description(&self) -> String {
        let descriptions: Vec<String> = self.filters.iter().map(|f| f.description()).collect();
        format!("({})", descriptions.join(" OR "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_entry(message: &str) -> LogEntry {
        LogEntry {
            pod_name: "test-pod".to_string(),
            raw_line: message.to_string(),
            message: message.to_string(),
        }
    }

    #[test]
    fn test_grep_filter() {
        let filter = GrepFilter::new("ERROR").unwrap();

        assert!(filter.apply(&create_test_entry("This is an ERROR message")));
        assert!(!filter.apply(&create_test_entry("This is a normal message")));
    }

    #[test]
    fn test_level_filter() {
        let filter = LevelFilter::new("ERROR").unwrap();

        assert!(filter.apply(&create_test_entry("[ERROR] This is an error")));
        assert!(filter.apply(&create_test_entry("ERROR: This is an error message")));
        assert!(!filter.apply(&create_test_entry("[INFO] This is info")));
        assert!(!filter.apply(&create_test_entry("[DEBUG] This is debug")));

        // Test logs with no recognizable level - should be excluded
        assert!(!filter.apply(&create_test_entry("This log has no level")));
        
        // Test with different case
        assert!(filter.apply(&create_test_entry("[error] This is an error message")));
        
        // Test with word "error" in the middle of text - should match if it's a whole word
        assert!(filter.apply(&create_test_entry("This is an error message")));
        
        // Test with "error" as part of another word - should not match
        assert!(!filter.apply(&create_test_entry("No errorenous conditions detected")));
        
        // Test with "error" in a trace message - should not match if not a level indicator
        let trace_message = "Trace[1292960837]: ---\"Objects listed\" error:Get";
        assert!(!filter.apply(&create_test_entry(trace_message)));
    }

    #[test]
    fn test_and_filter() {
        let grep1 = Box::new(GrepFilter::new("important").unwrap());
        let grep2 = Box::new(GrepFilter::new("ERROR").unwrap());

        let and_filter = AndFilter::new(vec![grep1, grep2]);

        assert!(and_filter.apply(&create_test_entry("This is an important ERROR message")));
        assert!(!and_filter.apply(&create_test_entry("This is an important message")));
        assert!(!and_filter.apply(&create_test_entry("This is an ERROR message")));
    }

    #[test]
    fn test_or_filter() {
        let grep1 = Box::new(GrepFilter::new("important").unwrap());
        let grep2 = Box::new(GrepFilter::new("ERROR").unwrap());

        let or_filter = OrFilter::new(vec![grep1, grep2]);

        assert!(or_filter.apply(&create_test_entry("This is an important ERROR message")));
        assert!(or_filter.apply(&create_test_entry("This is an important message")));
        assert!(or_filter.apply(&create_test_entry("This is an ERROR message")));
        assert!(!or_filter.apply(&create_test_entry("This is a normal message")));
    }

    #[test]
    fn test_numeric_grep_filter() {
        let filter = GrepFilter::new("123").unwrap();
        
        assert!(filter.apply(&create_test_entry("Log with number 123")));
        assert!(filter.apply(&create_test_entry("Log with number 1234")));
        assert!(!filter.apply(&create_test_entry("Log with number 456")));
    }
}
