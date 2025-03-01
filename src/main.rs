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
        
        // 检查是否包含 & 分隔符
        let is_and_operation = grep.contains('&');
        
        // 根据分隔符选择分割模式
        let patterns: Vec<&str> = if is_and_operation {
            grep.split('&').collect()
        } else {
            // 默认使用逗号分隔，保持向后兼容
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
                        // 只记录详细错误，不在日志中重复输出用户将看到的错误信息
                        debug!("Failed to create grep filter for pattern '{}': {}", trimmed_pattern, e);
                        return Err(anyhow!("Invalid grep pattern: {}", trimmed_pattern));
                    }
                }
            }
        }
        
        if !grep_filters.is_empty() {
            // 根据分隔符决定使用 AND 还是 OR 逻辑
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
        // 使用逗号分隔，表示 OR 逻辑
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
                        // 只记录详细错误，不在日志中重复输出用户将看到的错误信息
                        debug!("Invalid level '{}': {}", trimmed_level, e);
                        return Err(anyhow!("Invalid log level: {}", trimmed_level));
                    }
                }
            }
        }
        
        if !level_filters.is_empty() {
            // 始终使用 OR 逻辑组合日志级别
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
            info!(
                "Displaying the last {} log entries and following new ones",
                tail_count
            );
            run_with_tail_follow(
                client,
                &pods,
                tail_count,
                &filters,
                &mut formatter,
                args.follow,
            )
            .await
        } else {
            // Non-follow mode with tail parameter
            info!("Displaying the last {} log entries", tail_count);
            run_with_tail_no_follow(client, &pods, tail_count, &filters, &mut formatter)
                .await
        }
    } else {
        // No tail parameter
        run_standard(client, &pods, &filters, &mut formatter, args.follow).await
    }
}

// Run with tail parameter in non-follow mode
async fn run_with_tail_no_follow(
    client: KubeClient,
    pods: &[PodInfo],
    tail_count: usize,
    filters: &[Box<dyn Filter>],
    formatter: &mut LogFormatter,
) -> Result<()> {
    // Create a buffer for each pod
    let mut pod_buffers: HashMap<String, Vec<LogEntry>> = HashMap::new();

    // Initialize buffers for each pod
    for pod in pods {
        pod_buffers.insert(pod.name.clone(), Vec::with_capacity(tail_count));
    }

    // Create log aggregator
    let mut aggregator = LogAggregator::new();

    // Add pod log streams to aggregator - use kubectl's built-in tail parameter
    for pod in pods {
        debug!("Adding log stream for pod {} ({})", pod.name, pod.status);
        let log_stream = client.get_pod_logs(pod, false, Some(tail_count)).await?;
        aggregator.add_pod_stream(pod.clone(), log_stream).await?;
    }

    // Get log stream
    let mut log_stream = aggregator.stream();

    // Collect logs for each pod
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

    Ok(())
}

// Run with tail parameter in follow mode
async fn run_with_tail_follow(
    client: KubeClient,
    pods: &[PodInfo],
    tail_count: usize,
    filters: &[Box<dyn Filter>],
    formatter: &mut LogFormatter,
    follow: bool,
) -> Result<()> {
    // Create log aggregator
    let mut aggregator = LogAggregator::new();

    // Add pod log streams to aggregator - use kubectl's built-in tail parameter
    for pod in pods {
        debug!("Adding log stream for pod {} ({})", pod.name, pod.status);
        // We'll use kubectl's --tail parameter to limit initial logs
        let log_stream = client.get_pod_logs(pod, follow, Some(tail_count)).await?;
        aggregator.add_pod_stream(pod.clone(), log_stream).await?;
    }

    // Get log stream
    let mut log_stream = aggregator.stream();

    // Process logs - in follow mode, we display logs as they come in
    info!(
        "Displaying the last {} log entries per pod and following new ones",
        tail_count
    );

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

    Ok(())
}

// Run standard mode (no tail parameter)
async fn run_standard(
    client: KubeClient,
    pods: &[PodInfo],
    filters: &[Box<dyn Filter>],
    formatter: &mut LogFormatter,
    follow: bool,
) -> Result<()> {
    // Create log aggregator
    let mut aggregator = LogAggregator::new();

    // Add pod log streams to aggregator
    for pod in pods {
        debug!("Adding log stream for pod {} ({})", pod.name, pod.status);
        let log_stream = client.get_pod_logs(pod, follow, None).await?;
        aggregator.add_pod_stream(pod.clone(), log_stream).await?;
    }

    // Get log stream
    let mut log_stream = aggregator.stream();

    // Process and display logs
    while let Some(entry_result) = log_stream.recv().await {
        match entry_result {
            Ok(entry) => {
                // Apply filters
                if !filters.is_empty() && !filters.iter().all(|f| f.apply(&entry)) {
                    continue;
                }

                // Display log
                if let Err(e) = formatter.format_colored(&entry) {
                    error!("Failed to format log entry: {}", e);
                }
            }
            Err(e) => {
                error!("Error receiving log entry: {}", e);
            }
        }
    }

    Ok(())
}
