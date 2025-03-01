use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use tokio::sync::mpsc;
use tokio::task;
use tracing::{debug, error, warn};

use crate::kubernetes::log::LogEntry;

/// External command pipe for processing log entries
pub struct CommandPipe {
    command: String,
}

impl CommandPipe {
    /// Create a new command pipe
    pub fn new(command: &str) -> Self {
        Self {
            command: command.to_string(),
        }
    }

    /// Process a log entry through the command pipe
    pub fn process(&self, entry: &LogEntry) -> Result<String> {
        // Create a command with shell
        #[cfg(target_os = "windows")]
        let mut cmd = Command::new("cmd");
        #[cfg(target_os = "windows")]
        cmd.args(["/C", &self.command]);

        #[cfg(not(target_os = "windows"))]
        let mut cmd = Command::new("sh");
        #[cfg(not(target_os = "windows"))]
        cmd.args(["-c", &self.command]);

        // Set up pipes
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Start the command
        let mut child = cmd
            .spawn()
            .context(format!("Failed to start command: {}", self.command))?;

        // Write the log message to stdin
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(entry.raw_line.as_bytes())
                .context("Failed to write to command stdin")?;
            // Close stdin to signal EOF
            drop(stdin);
        }

        // Read stdout
        let stdout = child
            .stdout
            .take()
            .context("Failed to open command stdout")?;
        let reader = BufReader::new(stdout);
        let mut output = String::new();

        for line in reader.lines() {
            match line {
                Ok(line) => {
                    output.push_str(&line);
                    output.push('\n');
                }
                Err(e) => {
                    warn!("Error reading command output: {}", e);
                }
            }
        }

        // Wait for the command to finish
        let status = child.wait().context("Failed to wait for command")?;

        if !status.success() {
            // Read stderr if command failed
            if let Some(stderr) = child.stderr.take() {
                let reader = BufReader::new(stderr);
                let mut error_output = String::new();

                for line in reader.lines() {
                    if let Ok(line) = line {
                        error_output.push_str(&line);
                        error_output.push('\n');
                    }
                }

                warn!("Command failed with status {}: {}", status, error_output);
            }

            return Err(anyhow::anyhow!("Command failed with status {}", status));
        }

        // Trim trailing newline
        if output.ends_with('\n') {
            output.pop();
        }

        Ok(output)
    }

    /// Process log entries asynchronously
    pub async fn process_stream(
        self,
        mut input_rx: mpsc::Receiver<Result<LogEntry>>,
        output_tx: mpsc::Sender<Result<LogEntry>>,
    ) {
        // Spawn a blocking task for command execution
        let _ = task::spawn_blocking(move || {
            while let Some(entry_result) = input_rx.blocking_recv() {
                match entry_result {
                    Ok(entry) => {
                        match self.process(&entry) {
                            Ok(processed_message) => {
                                // Create a new log entry with processed message
                                let processed_entry = LogEntry {
                                    pod_name: entry.pod_name,
                                    raw_line: processed_message.clone(),
                                    timestamp: entry.timestamp,
                                    message: processed_message,
                                };

                                if let Err(e) = output_tx.blocking_send(Ok(processed_entry)) {
                                    error!("Failed to send processed log entry: {}", e);
                                    break;
                                }
                            }
                            Err(e) => {
                                if let Err(e) = output_tx.blocking_send(Err(e)) {
                                    error!("Failed to send error: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        if let Err(e) = output_tx.blocking_send(Err(e)) {
                            error!("Failed to send error: {}", e);
                            break;
                        }
                    }
                }
            }

            debug!("Command pipe task ended");
        });

        // We don't need to await the handle here as it will run in the background
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_pipe_echo() {
        let pipe = CommandPipe::new("echo test");
        let entry = LogEntry {
            pod_name: "test-pod".to_string(),
            raw_line: "Hello, world!".to_string(),
            timestamp: None,
            message: "Hello, world!".to_string(),
        };

        let result = pipe.process(&entry).unwrap();
        assert_eq!(result, "test");
    }

    #[test]
    fn test_command_pipe_cat() {
        let pipe = CommandPipe::new("cat");
        let entry = LogEntry {
            pod_name: "test-pod".to_string(),
            raw_line: "Hello, world!".to_string(),
            timestamp: None,
            message: "Hello, world!".to_string(),
        };

        let result = pipe.process(&entry).unwrap();
        assert_eq!(result, "Hello, world!");
    }
}
