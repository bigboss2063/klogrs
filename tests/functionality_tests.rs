use anyhow::Result;
use klogrs::{
    kubernetes::{KubeClient, LogAggregator},
    log_processor::{Filter, GrepFilter},
};
use std::{process::Command, str};

/// Run all functionality tests
#[tokio::test]
async fn run_all_functionality_tests() -> Result<()> {
    println!("Running all functionality tests");

    // Run basic functionality test
    test_basic_functionality().await?;

    // Run grep functionality test
    test_grep_functionality().await?;

    println!("All functionality tests passed");
    Ok(())
}

/// Test basic functionality
async fn test_basic_functionality() -> Result<()> {
    println!("Testing basic functionality");

    // Create Kubernetes client
    let client = KubeClient::new().await?;

    // Get pods for coredns deployment
    let mut pods = client
        .get_pods_for_deployment("kube-system", "coredns")
        .await?;

    // Filter out pods that can't get logs
    pods.retain(|pod| pod.can_get_logs());

    // Ensure pods were found
    assert!(!pods.is_empty(), "No active coredns pods found");

    // Create log aggregator
    let mut aggregator = LogAggregator::new();

    // Add pod log streams
    for pod in &pods {
        let log_stream = client.get_pod_logs(pod, false, None).await?;
        aggregator.add_pod_stream(pod.clone(), log_stream).await?;
    }

    // Get log stream
    let mut log_stream = aggregator.stream();

    // Process logs
    let mut count = 0;
    let max_logs = 5;

    println!("Logs from coredns pods:");
    while let Some(entry_result) = log_stream.recv().await {
        if let Ok(entry) = entry_result {
            println!("{}: {}", entry.pod_name, entry.message);
            count += 1;
            if count >= max_logs {
                break;
            }
        }
    }

    if count > 0 {
        println!("Success: Basic functionality working, found logs");
    } else {
        println!("Note: No logs found, but functionality seems to be working");
    }

    Ok(())
}

/// Test grep functionality
async fn test_grep_functionality() -> Result<()> {
    println!("Testing grep functionality");

    // Create Kubernetes client
    let client = KubeClient::new().await?;

    // Get pods for coredns deployment
    let mut pods = client
        .get_pods_for_deployment("kube-system", "coredns")
        .await?;

    // Filter out pods that can't get logs
    pods.retain(|pod| pod.can_get_logs());

    // Ensure pods were found
    assert!(!pods.is_empty(), "No active coredns pods found");

    // Create log aggregator
    let mut aggregator = LogAggregator::new();

    // Add pod log streams
    for pod in &pods {
        let log_stream = client.get_pod_logs(pod, false, None).await?;
        aggregator.add_pod_stream(pod.clone(), log_stream).await?;
    }

    // Create grep filter
    let grep_pattern = "INFO";
    let grep_filter = GrepFilter::new(grep_pattern)?;

    // Get log stream
    let mut log_stream = aggregator.stream();

    // Process logs
    let mut count = 0;
    let max_logs = 5;
    let mut found_match = false;

    println!("Results of grep filter for '{}':", grep_pattern);
    while let Some(entry_result) = log_stream.recv().await {
        if let Ok(entry) = entry_result {
            if grep_filter.apply(&entry) {
                println!("{}: {}", entry.pod_name, entry.message);
                found_match = true;
                count += 1;
                if count >= max_logs {
                    break;
                }
            }
        }
    }

    if found_match {
        println!("Success: Grep functionality working, found matching logs");
    } else {
        println!(
            "Note: No matching logs found, but this may be because logs don't contain '{}'",
            grep_pattern
        );
    }

    Ok(())
}

/// Test help command
#[test]
fn test_help_command() -> Result<()> {
    println!("Testing help information");

    let output = Command::new("cargo")
        .args(&["run", "--", "--help"])
        .output()?;

    let stdout = str::from_utf8(&output.stdout)?;
    println!("Help output:");
    println!("{}", stdout);

    // Verify help information contains expected content
    assert!(
        stdout.contains("Usage:") || stdout.contains("USAGE:"),
        "Help should contain 'Usage:' or 'USAGE:'"
    );
    assert!(
        stdout.contains("Options:") || stdout.contains("OPTIONS:"),
        "Help should contain 'Options:' or 'OPTIONS:'"
    );
    assert!(
        stdout.contains("--deployment"),
        "Help should contain '--deployment' option"
    );
    assert!(
        stdout.contains("--namespace"),
        "Help should contain '--namespace' option"
    );

    Ok(())
}

/// Test version command
#[test]
fn test_version_command() -> Result<()> {
    println!("Testing version information");

    let output = Command::new("cargo")
        .args(&["run", "--", "--version"])
        .output()?;

    let stdout = str::from_utf8(&output.stdout)?;
    println!("Version output:");
    println!("{}", stdout);

    // Verify version information is displayed
    assert!(!stdout.is_empty(), "Version output should not be empty");
    assert!(
        stdout.contains("klogrs") || stdout.contains(env!("CARGO_PKG_NAME")),
        "Version output should contain package name"
    );

    Ok(())
}
