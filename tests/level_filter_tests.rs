use anyhow::Result;
use klogrs::{
    cli::parse_args,
    kubernetes::log::LogEntry,
    log_processor::filter::{Filter, LevelFilter},
};
use std::ffi::OsString;

/// Test level filter command line argument
#[test]
fn test_level_arg() -> Result<()> {
    let args = parse_args(vec![
        OsString::from("klogrs"),
        OsString::from("-n"),
        OsString::from("default"),
        OsString::from("-d"),
        OsString::from("nginx"),
        OsString::from("-l"),
        OsString::from("WARN"),
    ])?;

    assert_eq!(args.level, Some("WARN".to_string()));
    Ok(())
}

/// Test level filter functionality with different log formats
#[test]
fn test_level_filter_with_different_formats() -> Result<()> {
    // 创建一个 WARN 级别的过滤器
    let warn_filter = LevelFilter::new("WARN")?;
    
    // 创建一个 ERROR 级别的过滤器
    let error_filter = LevelFilter::new("ERROR")?;

    // 创建测试日志条目
    let entries = vec![
        create_test_entry("[ERROR] Critical error occurred"),
        create_test_entry("[WARN] Warning message"),
        create_test_entry("[INFO] Informational message"),
        create_test_entry("[DEBUG] Debug information"),
        create_test_entry("ERROR: System failure"),
        create_test_entry("WARN: Configuration issue"),
        create_test_entry("INFO: Operation completed"),
        create_test_entry("2023-05-01 ERROR: Database connection lost"),
        create_test_entry("2023-05-01 WARN: Slow query detected"),
        create_test_entry("2023-05-01 INFO: User logged in"),
        create_test_entry("This message has the word ERROR in it"),
        create_test_entry("This message has the word WARNING in it"),
        create_test_entry("This message has the word INFO in it"),
        create_test_entry("No level specified"),
        create_test_entry("Trace[1292960837]: ---\"Objects listed\" error:Get"),
        create_test_entry("No errorenous conditions detected"),
    ];

    // 预期结果 - WARN 过滤器
    let expected_warn = vec![
        false, // [ERROR]
        true,  // [WARN]
        false, // [INFO]
        false, // [DEBUG]
        false, // ERROR:
        true,  // WARN:
        false, // INFO:
        false, // 2023-05-01 ERROR:
        true,  // 2023-05-01 WARN:
        false, // 2023-05-01 INFO:
        false, // Message containing ERROR
        true,  // Message containing WARNING
        false, // Message containing INFO
        false, // No level
        false, // Trace[...] with error
        false, // errorenous
    ];
    
    // 预期结果 - ERROR 过滤器
    let expected_error = vec![
        true,  // [ERROR]
        false, // [WARN]
        false, // [INFO]
        false, // [DEBUG]
        true,  // ERROR:
        false, // WARN:
        false, // INFO:
        true,  // 2023-05-01 ERROR:
        false, // 2023-05-01 WARN:
        false, // 2023-05-01 INFO:
        true,  // Message containing ERROR
        false, // Message containing WARNING
        false, // Message containing INFO
        false, // No level
        false, // Trace[...] with error
        false, // errorenous
    ];

    // 测试每个条目与 WARN 过滤器
    for (i, entry) in entries.iter().enumerate() {
        assert_eq!(
            warn_filter.apply(entry),
            expected_warn[i],
            "Failed on WARN filter entry: {}",
            entry.message
        );
    }
    
    // 测试每个条目与 ERROR 过滤器
    for (i, entry) in entries.iter().enumerate() {
        assert_eq!(
            error_filter.apply(entry),
            expected_error[i],
            "Failed on ERROR filter entry: {}",
            entry.message
        );
    }

    Ok(())
}

/// Helper function to create a test log entry
fn create_test_entry(message: &str) -> LogEntry {
    LogEntry {
        pod_name: "test-pod".to_string(),
        raw_line: message.to_string(),
        message: message.to_string(),
    }
}
