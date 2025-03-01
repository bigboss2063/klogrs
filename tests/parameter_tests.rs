use klogrs::cli::parse_args;
use std::ffi::OsString;

/// Test running with no arguments
#[test]
fn test_no_args() {
    let args = vec![OsString::from("klogrs")];
    let result = parse_args(args);
    assert!(result.is_err());
}

/// Test with minimal arguments
#[test]
fn test_minimal_args() {
    let args = vec![
        OsString::from("klogrs"),
        OsString::from("--deployment"),
        OsString::from("nginx"),
    ];
    let result = parse_args(args);
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.namespace, "default");
    assert_eq!(parsed.deployment, "nginx");
    assert_eq!(parsed.follow, false);
    assert_eq!(parsed.grep, None);
    assert_eq!(parsed.tail, None);
}

/// Test namespace argument
#[test]
fn test_namespace_arg() {
    let args = vec![
        OsString::from("klogrs"),
        OsString::from("--namespace"),
        OsString::from("kube-system"),
        OsString::from("--deployment"),
        OsString::from("nginx"),
    ];
    let result = parse_args(args);
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.namespace, "kube-system");
}

/// Test follow logs (--follow)
#[test]
fn test_follow_arg() {
    let args = vec![
        OsString::from("klogrs"),
        OsString::from("--namespace"),
        OsString::from("default"),
        OsString::from("--deployment"),
        OsString::from("nginx"),
        OsString::from("--follow"),
    ];
    let result = parse_args(args).unwrap();
    assert_eq!(result.follow, true);
}

/// Test follow logs (-f)
#[test]
fn test_follow_short_arg() {
    let args = vec![
        OsString::from("klogrs"),
        OsString::from("--namespace"),
        OsString::from("default"),
        OsString::from("--deployment"),
        OsString::from("nginx"),
        OsString::from("-f"),
    ];
    let result = parse_args(args).unwrap();
    assert_eq!(result.follow, true);
}

/// Test grep filter (--grep)
#[test]
fn test_grep_arg() {
    let args = vec![
        OsString::from("klogrs"),
        OsString::from("-d"),
        OsString::from("nginx"),
        OsString::from("--grep"),
        OsString::from("ERROR"),
    ];
    let result = parse_args(args).unwrap();
    assert_eq!(result.grep, Some("ERROR".to_string()));
}

/// Test grep filter (-g)
#[test]
fn test_grep_short_arg() {
    let args = vec![
        OsString::from("klogrs"),
        OsString::from("-d"),
        OsString::from("nginx"),
        OsString::from("-g"),
        OsString::from("ERROR"),
    ];
    let result = parse_args(args).unwrap();
    assert_eq!(result.grep, Some("ERROR".to_string()));
}

/// Test combined arguments
#[test]
fn test_combined_args() {
    let args = vec![
        OsString::from("klogrs"),
        OsString::from("-n"),
        OsString::from("default"),
        OsString::from("-d"),
        OsString::from("nginx"),
        OsString::from("-f"),
        OsString::from("-g"),
        OsString::from("ERROR"),
    ];
    let result = parse_args(args).unwrap();
    assert_eq!(result.namespace, "default");
    assert_eq!(result.deployment, "nginx");
    assert_eq!(result.follow, true);
    assert_eq!(result.grep, Some("ERROR".to_string()));
    assert_eq!(result.tail, None);
}

/// Test tail parameter (--tail)
#[test]
fn test_tail_arg() {
    let args = vec![
        OsString::from("klogrs"),
        OsString::from("-d"),
        OsString::from("nginx"),
        OsString::from("--tail"),
        OsString::from("50"),
    ];
    let result = parse_args(args).unwrap();
    assert_eq!(result.tail, Some(50));
}

/// Test tail parameter (-t)
#[test]
fn test_tail_short_arg() {
    let args = vec![
        OsString::from("klogrs"),
        OsString::from("-d"),
        OsString::from("nginx"),
        OsString::from("-t"),
        OsString::from("10"),
    ];
    let result = parse_args(args).unwrap();
    assert_eq!(result.tail, Some(10));
}

/// Test parameter parsing
#[test]
fn test_parse_args() {
    // Test with default namespace
    let args = vec![
        OsString::from("klogrs"),
        OsString::from("-d"),
        OsString::from("test-deployment"),
    ];
    let parsed = parse_args(args).unwrap();
    assert_eq!(parsed.namespace, "default");
    assert_eq!(parsed.deployment, "test-deployment");
    assert_eq!(parsed.follow, false);
    assert_eq!(parsed.grep, None);
    assert_eq!(parsed.tail, None);

    // Test with custom namespace
    let args = vec![
        OsString::from("klogrs"),
        OsString::from("-n"),
        OsString::from("test-namespace"),
        OsString::from("-d"),
        OsString::from("test-deployment"),
    ];
    let parsed = parse_args(args).unwrap();
    assert_eq!(parsed.namespace, "test-namespace");
    assert_eq!(parsed.deployment, "test-deployment");
    assert_eq!(parsed.follow, false);
    assert_eq!(parsed.grep, None);
    assert_eq!(parsed.tail, None);

    // Test with follow flag
    let args = vec![
        OsString::from("klogrs"),
        OsString::from("-d"),
        OsString::from("test-deployment"),
        OsString::from("-f"),
    ];
    let parsed = parse_args(args).unwrap();
    assert_eq!(parsed.namespace, "default");
    assert_eq!(parsed.deployment, "test-deployment");
    assert_eq!(parsed.follow, true);
    assert_eq!(parsed.grep, None);
    assert_eq!(parsed.tail, None);

    // Test with grep pattern
    let args = vec![
        OsString::from("klogrs"),
        OsString::from("-d"),
        OsString::from("test-deployment"),
        OsString::from("-g"),
        OsString::from("error"),
    ];
    let parsed = parse_args(args).unwrap();
    assert_eq!(parsed.namespace, "default");
    assert_eq!(parsed.deployment, "test-deployment");
    assert_eq!(parsed.follow, false);
    assert_eq!(parsed.grep, Some("error".to_string()));
    assert_eq!(parsed.tail, None);

    // Test with tail option
    let args = vec![
        OsString::from("klogrs"),
        OsString::from("-d"),
        OsString::from("test-deployment"),
        OsString::from("-t"),
        OsString::from("20"),
    ];
    let parsed = parse_args(args).unwrap();
    assert_eq!(parsed.namespace, "default");
    assert_eq!(parsed.deployment, "test-deployment");
    assert_eq!(parsed.follow, false);
    assert_eq!(parsed.grep, None);
    assert_eq!(parsed.tail, Some(20));

    // Test with all options
    let args = vec![
        OsString::from("klogrs"),
        OsString::from("-n"),
        OsString::from("test-namespace"),
        OsString::from("-d"),
        OsString::from("test-deployment"),
        OsString::from("-f"),
        OsString::from("-g"),
        OsString::from("error"),
        OsString::from("-t"),
        OsString::from("30"),
    ];
    let parsed = parse_args(args).unwrap();
    assert_eq!(parsed.namespace, "test-namespace");
    assert_eq!(parsed.deployment, "test-deployment");
    assert_eq!(parsed.follow, true);
    assert_eq!(parsed.grep, Some("error".to_string()));
    assert_eq!(parsed.tail, Some(30));
}
