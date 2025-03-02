use anyhow::{Result, anyhow};
use klogrs::{
    cli::{parse_args, Args},
    kubernetes::{KubeClient, LogAggregator, LogEntry, PodInfo},
    log_processor::{
        filter::{AndFilter, Filter, GrepFilter, LevelFilter, OrFilter},
        LogFormatter,
    },
};
use std::collections::HashMap;
use std::env;
use tracing::{debug, error, info};
use tracing_subscriber::fmt::format::FmtSpan;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "warn".to_string()))
        .with_span_events(FmtSpan::CLOSE)
        .init();

    // Parse command-line arguments
    let args = parse_args(env::args_os())?;

    // Run the application
    run(args).await
}

async fn run(args: Args) -> Result<()> {
    // Create Kubernetes client
    let client = KubeClient::new().await?;

    // Get pods for deployment
    let mut pods = client
        .get_pods_for_deployment(&args.namespace, &args.deployment)
        .await?;

    // Filter out terminated pods
    pods.retain(|pod| pod.can_get_logs());

    if pods.is_empty() {
        return Err(anyhow::anyhow!(
            "No active pods found for deployment {}",
            args.deployment
        ));
    }

    info!(
        "Found {} active pods for deployment {}",
        pods.len(),
        args.deployment
    );

    // Create log formatter
    let mut formatter = LogFormatter::new(None, false);
    
    // Set highlight option
    formatter.set_highlight(!args.no_highlight);
    
    // Create filters
    let mut filters: Vec<Box<dyn Filter>> = Vec::new();
    let mut combined_filters: Vec<Box<dyn Filter>> = Vec::new();

    // Handle grep filter
    if let Some(grep) = &args.grep {
        let mut grep_filters: Vec<Box<dyn Filter>> = Vec::new();
        
        // Check if it contains the & separator
        let is_and_operation = grep.contains('&');
        
        // Select the split mode according to the separator
        let patterns: Vec<&str> = if is_and_operation {
            grep.split('&').collect()
        } else {
            // Use comma as the separator by default to maintain backward compatibility
            grep.split(',').collect()
        };

        for pattern in patterns {
            let trimmed_pattern = pattern.trim();
            if !trimmed_pattern.is_empty() {
                match GrepFilter::new(trimmed_pattern) {
                    Ok(filter) => {
                        // Add grep filter to formatter for highlighting if highlighting is enabled
                        if !args.no_highlight {
                            formatter.add_grep_filter(filter.clone());
                        }
                        
                        grep_filters.push(Box::new(filter));
                    }
                    Err(e) => {
                        // Only record detailed errors, and do not repeat the error messages that users will see in the logs
                        debug!("Failed to create grep filter for pattern '{}': {}", trimmed_pattern, e);
                        return Err(anyhow!("Invalid grep pattern: {}", trimmed_pattern));
                    }
                }
            }
        }
        
        if !grep_filters.is_empty() {
            // Determine whether to use AND or OR logic according to the separator
            if is_and_operation {
                info!("Filtering logs with ALL of the patterns: {}", grep);
                combined_filters.push(Box::new(AndFilter::new(grep_filters)));
            } else {
                info!("Filtering logs with ANY of the patterns: {}", grep);
                combined_filters.push(Box::new(OrFilter::new(grep_filters)));
            }
        }
    }

    // Handle level filter
    if let Some(level) = args.level.as_ref() {
        // Use comma as the separator, indicating OR logic
        let level_values: Vec<&str> = level.split(',').collect();
        
        let mut level_filters: Vec<Box<dyn Filter>> = Vec::new();

        for level_value in level_values {
            let trimmed_level = level_value.trim();
            if !trimmed_level.is_empty() {
                match LevelFilter::new(trimmed_level) {
                    Ok(filter) => {
                        info!("Adding level filter for: '{}'", trimmed_level);
                        level_filters.push(Box::new(filter));
                    },
                    Err(e) => {
                        // Only record detailed errors, and do not repeat the error messages that users will see in the logs
                        debug!("Invalid level '{}': {}", trimmed_level, e);
                        return Err(anyhow!("Invalid log level: {}", trimmed_level));
                    }
                }
            }
        }
        
        if !level_filters.is_empty() {
            // Always use OR logic to combine log levels
            info!("Filtering logs with ANY of the levels: {}", level);
            combined_filters.push(Box::new(OrFilter::new(level_filters)));
        }
    }
    
    // If we have multiple filter types (grep and level), combine them with AND logic
    if combined_filters.len() > 1 {
        info!("Using AND logic to combine grep and level filters");
        filters.push(Box::new(AndFilter::new(combined_filters)));
    } else if combined_filters.len() == 1 {
        // Just one filter type, add it directly
        filters.extend(combined_filters);
    }

    // Handle tail parameter
    if let Some(tail_count) = args.tail {
        if args.follow {
            // Follow mode with tail parameter
            run_logs(client, &pods, &filters, &mut formatter, true, Some(tail_count)).await
        } else {
            // Non-follow mode with tail parameter
            run_logs(client, &pods, &filters, &mut formatter, false, Some(tail_count)).await
        }
    } else {
        // No tail parameter
        run_logs(client, &pods, &filters, &mut formatter, args.follow, None).await
    }
}

// Unified log running function, replacing the previous three functions
async fn run_logs(
    client: KubeClient,
    pods: &[PodInfo],
    filters: &[Box<dyn Filter>],
    formatter: &mut LogFormatter,
    follow: bool,
    tail: Option<usize>,
) -> Result<()> {
    // Log mode information
    match (follow, tail) {
        (true, Some(count)) => {
            info!(
                "Displaying the last {} log entries and following new ones",
                count
            );
        }
        (false, Some(count)) => {
            info!("Displaying the last {} log entries", count);
        }
        (true, None) => {
            info!("Following logs in real-time");
        }
        (false, None) => {
            info!("Displaying all available logs");
        }
    }

    // Create log aggregator
    let mut aggregator = LogAggregator::new();

    // Prepare to get log streams in parallel
    let mut handles = Vec::with_capacity(pods.len());
    
    // Get log streams for each pod in parallel
    for pod in pods {
        let client_clone = client.clone();
        let pod_clone = pod.clone();
        let follow_clone = follow;
        let tail_clone = tail;
        
        debug!("Starting log stream task for pod {} ({})", pod.name, pod.status);
        
        // Create an asynchronous task for each pod
        let handle = tokio::spawn(async move {
            let result = client_clone.get_pod_logs(&pod_clone, follow_clone, tail_clone).await;
            (pod_clone, result)
        });
        
        handles.push(handle);
    }

    // Wait for all log streams to initialize and add them to the aggregator
    for handle in handles {
        match handle.await {
            Ok((pod, log_stream_result)) => {
                match log_stream_result {
                    Ok(log_stream) => {
                        debug!("Adding log stream for pod {} ({})", pod.name, pod.status);
                        if let Err(e) = aggregator.add_pod_stream(pod, log_stream).await {
                            error!("Failed to add pod stream: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to get logs for pod {}: {}", pod.name, e);
                    }
                }
            }
            Err(e) => {
                error!("Task failed: {}", e);
            }
        }
    }

    // Get the log stream
    let mut log_stream = aggregator.stream();

    if !follow && tail.is_some() {
        let tail_count = tail.unwrap();
        let mut pod_buffers: HashMap<String, Vec<LogEntry>> = HashMap::new();

        for pod in pods {
            pod_buffers.insert(pod.name.clone(), Vec::with_capacity(tail_count));
        }

        // Buffer logs for each pod
        while let Some(entry_result) = log_stream.recv().await {
            match entry_result {
                Ok(entry) => {
                    // Apply filters
                    if !filters.is_empty() && !filters.iter().all(|f| f.apply(&entry)) {
                        continue;
                    }

                    // Add to the appropriate pod buffer
                    if let Some(buffer) = pod_buffers.get_mut(&entry.pod_name) {
                        buffer.push(entry);
                    }
                }
                Err(e) => {
                    error!("Error receiving log entry: {}", e);
                }
            }
        }

        // Display the buffered logs for each pod
        for (pod_name, buffer) in pod_buffers {
            if !buffer.is_empty() {
                info!("Logs for pod {}:", pod_name);

                // If we have more logs than tail_count, only show the last tail_count logs
                let logs_to_display = if buffer.len() > tail_count {
                    &buffer[buffer.len() - tail_count..]
                } else {
                    &buffer[..]
                };

                for entry in logs_to_display {
                    if let Err(e) = formatter.format_colored(entry) {
                        error!("Failed to format log entry: {}", e);
                    }
                }
            }
        }
    } else {
        // Display logs in real-time
        while let Some(entry_result) = log_stream.recv().await {
            match entry_result {
                Ok(entry) => {
                    // Apply filters
                    if !filters.is_empty() && !filters.iter().all(|f| f.apply(&entry)) {
                        continue;
                    }

                    // Display the log entry
                    if let Err(e) = formatter.format_colored(&entry) {
                        error!("Failed to format log entry: {}", e);
                    }
                }
                Err(e) => {
                    error!("Error receiving log entry: {}", e);
                }
            }
        }
    }

    Ok(())
}
