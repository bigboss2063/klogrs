use super::PodInfo;
use anyhow::Result;
use futures::{Stream, StreamExt};
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info};

/// Type alias for a boxed stream of log lines
pub type LogStream = Pin<Box<dyn Stream<Item = Result<Vec<u8>>> + Send>>;

/// Log entry with metadata
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Pod name
    pub pod_name: String,
    /// Raw log line
    pub raw_line: String,
    /// Log message (without timestamp)
    pub message: String,
}

impl LogEntry {
    /// Parse a raw log line into a LogEntry
    pub fn parse(pod_name: String, raw_line: String) -> Self {
        // Clean the line
        let clean_line = raw_line.replace('\r', "").replace('\0', "");

        // Try to extract the message without timestamp
        // Kubernetes log timestamp format is typically: YYYY-MM-DDTHH:MM:SS.sssssssssZ
        let message = if let Some(timestamp_end) = clean_line.find(" ") {
            // If we find a space after the timestamp, extract everything after it
            clean_line[timestamp_end + 1..].to_string()
        } else {
            // If we can't find a timestamp pattern, use the whole line
            clean_line.clone()
        };

        Self {
            pod_name,
            raw_line: clean_line,
            message,
        }
    }
}

/// Log aggregator for multiple pods
pub struct LogAggregator {
    /// Channel for receiving log entries
    rx: mpsc::Receiver<Result<LogEntry>>,
    /// Sender channel for aggregated log entries
    tx: mpsc::Sender<Result<LogEntry>>,
}

impl LogAggregator {
    /// Create a new log aggregator
    pub fn new() -> Self {
        // Reduce buffer size to minimize latency
        let (tx, rx) = mpsc::channel(100);

        Self { rx, tx }
    }

    /// Add a pod log stream to the aggregator
    pub async fn add_pod_stream(
        &mut self,
        pod_info: PodInfo,
        mut log_stream: LogStream,
    ) -> Result<()> {
        let tx = self.tx.clone();
        let pod_name = pod_info.name.clone();

        // Spawn a task to process this pod's logs
        tokio::spawn(async move {
            while let Some(line_result) = log_stream.next().await {
                match line_result {
                    Ok(bytes) => {
                        // Convert bytes to string
                        let line_str = String::from_utf8_lossy(&bytes).to_string();

                        // Process the line
                        debug!("Received log line from pod {}: {}", pod_name, line_str);

                        // Create log entry
                        let entry = LogEntry::parse(pod_name.clone(), line_str);

                        // Send to channel with minimal delay
                        if let Err(e) = tx.send(Ok(entry)).await {
                            error!("Failed to send log entry: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        if let Err(e) = tx
                            .send(Err(anyhow::anyhow!("Error from pod {}: {}", pod_name, e)))
                            .await
                        {
                            error!("Failed to send error: {}", e);
                        }
                        // Reduce wait time before reconnecting
                        sleep(Duration::from_millis(500)).await;
                    }
                }
            }

            info!("Log stream for pod {} ended", pod_name);
        });

        Ok(())
    }

    /// Get the stream of aggregated log entries
    pub fn stream(self) -> mpsc::Receiver<Result<LogEntry>> {
        self.rx
    }
}
