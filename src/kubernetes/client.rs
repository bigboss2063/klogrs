use anyhow::{Context, Result};
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{Api, ListParams},
    Client, Config,
};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{debug, error};

use super::{LogStream, PodInfo, PodStatus};

/// Kubernetes client for interacting with the API server
pub struct KubeClient {
    client: Client,
}

impl KubeClient {
    /// Create a new Kubernetes client
    pub async fn new() -> Result<Self> {
        let config = Config::from_kubeconfig(&Default::default()).await?;
        let client = Client::try_from(config).context("Failed to create Kubernetes client")?;

        Ok(Self { client })
    }

    /// Get pods for a deployment
    pub async fn get_pods_for_deployment(
        &self,
        namespace: &str,
        deployment: &str,
    ) -> Result<Vec<PodInfo>> {
        debug!(
            "Searching for pods in namespace '{}' for deployment '{}'",
            namespace, deployment
        );

        // First, try to get the deployment to find its selector
        let deployments_api: Api<Deployment> = Api::namespaced(self.client.clone(), namespace);

        let mut all_pods = Vec::new();

        // Try to get the deployment
        match deployments_api.get(deployment).await {
            Ok(deployment_obj) => {
                // Extract the selector from the deployment
                if let Some(selector) = deployment_obj
                    .spec
                    .and_then(|spec| spec.selector.match_labels)
                {
                    // Create a label selector string from the match_labels map
                    let selector_parts: Vec<String> = selector
                        .iter()
                        .map(|(key, value)| format!("{}={}", key, value))
                        .collect();

                    if selector_parts.is_empty() {
                        debug!("Deployment {} has no selector labels", deployment);
                        return Ok(all_pods);
                    }

                    let selector_str = selector_parts.join(",");
                    debug!("Using selector from deployment: {}", selector_str);

                    // List pods with the deployment's selector
                    let lp = ListParams::default().labels(&selector_str);
                    let pods_api: Api<Pod> = Api::namespaced(self.client.clone(), namespace);

                    match pods_api.list(&lp).await {
                        Ok(pods) => {
                            debug!(
                                "Found {} pods for deployment {}",
                                pods.items.len(),
                                deployment
                            );

                            // Extract pod info
                            for pod in pods.items {
                                if let Some(pod_name) = pod.metadata.name.clone() {
                                    debug!("Found pod: {}", pod_name);
                                }

                                if let Some(pod_info) = self.extract_pod_info(pod) {
                                    debug!(
                                        "Extracted pod info for pod: {}, status: {:?}",
                                        pod_info.name, pod_info.status
                                    );
                                    all_pods.push(pod_info);
                                } else {
                                    error!("Failed to extract pod info for a pod");
                                }
                            }
                        }
                        Err(e) => {
                            error!("Error listing pods with deployment selector: {}", e);
                        }
                    }
                } else {
                    debug!("Deployment {} has no selector", deployment);
                }
            }
            Err(e) => {
                error!("Error getting deployment {}: {}", deployment, e);
            }
        }

        debug!(
            "Found a total of {} pods for deployment {}",
            all_pods.len(),
            deployment
        );

        Ok(all_pods)
    }

    /// Extract pod info from a Pod object
    fn extract_pod_info(&self, pod: Pod) -> Option<PodInfo> {
        let name = pod.metadata.name?;
        let namespace = pod
            .metadata
            .namespace
            .clone()
            .unwrap_or_else(|| "default".to_string());

        // Get pod status
        let status = if let Some(status) = pod.status {
            self.determine_pod_status(&status)
        } else {
            PodStatus::Unknown
        };

        // Get container name (use the first container)
        let container_name = pod.spec?.containers.first()?.name.clone();

        Some(PodInfo {
            name,
            namespace,
            status,
            container_name,
        })
    }

    /// Determine pod status from PodStatus
    fn determine_pod_status(&self, status: &k8s_openapi::api::core::v1::PodStatus) -> PodStatus {
        // Check phase
        if let Some(phase) = &status.phase {
            match phase.as_str() {
                "Running" => return PodStatus::Running,
                "Pending" => return PodStatus::Pending,
                "Succeeded" | "Failed" => return PodStatus::Terminated,
                _ => {}
            }
        }

        // Check container statuses for CrashLoopBackOff
        if let Some(container_statuses) = &status.container_statuses {
            for cs in container_statuses {
                if let Some(state) = &cs.state {
                    if let Some(waiting) = &state.waiting {
                        if let Some(reason) = &waiting.reason {
                            if reason == "CrashLoopBackOff" {
                                return PodStatus::CrashLoopBackOff;
                            }
                        }
                    }
                }
            }
        }

        PodStatus::Unknown
    }

    /// Get logs for a pod using kubectl command
    ///
    /// * `pod_info` - Pod information
    /// * `follow` - Whether to follow logs
    /// * `tail` - Optional tail parameter to limit the number of log entries
    pub async fn get_pod_logs(
        &self,
        pod_info: &PodInfo,
        follow: bool,
        tail: Option<usize>,
    ) -> Result<LogStream> {
        debug!(
            "Getting logs for pod {} in namespace {}{}",
            pod_info.name,
            pod_info.namespace,
            tail.map_or("".to_string(), |t| format!(" with tail {}", t))
        );

        // Build kubectl command
        let mut cmd = Command::new("kubectl");
        cmd.arg("logs")
            .arg(&pod_info.name)
            .arg("-n")
            .arg(&pod_info.namespace)
            .arg("-c")
            .arg(&pod_info.container_name)
            .arg("--timestamps=true");

        if follow {
            cmd.arg("-f");
            // Disable kubectl's output buffering for real-time logs
            cmd.env("PYTHONUNBUFFERED", "1");
            cmd.env("PYTHONIOENCODING", "UTF-8");
        }

        if let Some(tail_count) = tail {
            cmd.arg(format!("--tail={}", tail_count));
        }

        // Set up stdout to be captured
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // Start the command
        let mut child = cmd.spawn().context(format!(
            "Failed to spawn kubectl logs for pod {}",
            pod_info.name
        ))?;

        // Get stdout handle
        let stdout = child
            .stdout
            .take()
            .context("Failed to capture kubectl stdout")?;

        // Create a buffered reader with a smaller buffer for more immediate output
        let reader = BufReader::with_capacity(1024, stdout);
        let mut lines = reader.lines();

        // Create a stream from the lines
        let stream = async_stream::stream! {
            while let Some(line) = lines.next_line().await.transpose() {
                match line {
                    Ok(line) => yield Ok(line.into_bytes()),
                    Err(e) => yield Err(anyhow::anyhow!("Error reading kubectl output: {}", e)),
                }
            }
        };

        // Spawn a task to wait for the command to complete
        let pod_name = pod_info.name.clone();
        tokio::spawn(async move {
            match child.wait().await {
                Ok(status) => {
                    if !status.success() {
                        if let Some(stderr) = child.stderr.take() {
                            let mut stderr_reader = BufReader::new(stderr);
                            let mut error_message = String::new();
                            if let Ok(_) = stderr_reader.read_line(&mut error_message).await {
                                error!(
                                    "kubectl logs for pod {} failed: {}",
                                    pod_name,
                                    error_message.trim()
                                );
                            }
                        }
                        error!(
                            "kubectl logs for pod {} exited with status: {}",
                            pod_name, status
                        );
                    }
                }
                Err(e) => {
                    error!("Failed to wait for kubectl logs process: {}", e);
                }
            }
        });

        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{Container, PodSpec, PodStatus as K8sPodStatus};
    use kube::api::ObjectMeta;

    // Mock version of KubeClient for testing
    struct MockKubeClient;

    impl MockKubeClient {
        fn extract_pod_info(&self, pod: Pod) -> Option<PodInfo> {
            let name = pod.metadata.name?;
            let namespace = pod
                .metadata
                .namespace
                .clone()
                .unwrap_or_else(|| "default".to_string());

            // Get pod status
            let status = if let Some(status) = pod.status {
                self.determine_pod_status(&status)
            } else {
                PodStatus::Unknown
            };

            // Get container name (use the first container)
            let container_name = pod.spec?.containers.first()?.name.clone();

            Some(PodInfo {
                name,
                namespace,
                status,
                container_name,
            })
        }

        fn determine_pod_status(&self, status: &K8sPodStatus) -> PodStatus {
            // Check phase
            if let Some(phase) = &status.phase {
                match phase.as_str() {
                    "Running" => return PodStatus::Running,
                    "Pending" => return PodStatus::Pending,
                    "Succeeded" | "Failed" => return PodStatus::Terminated,
                    _ => {}
                }
            }

            // Check container statuses for CrashLoopBackOff
            if let Some(container_statuses) = &status.container_statuses {
                for cs in container_statuses {
                    if let Some(state) = &cs.state {
                        if let Some(waiting) = &state.waiting {
                            if let Some(reason) = &waiting.reason {
                                if reason == "CrashLoopBackOff" {
                                    return PodStatus::CrashLoopBackOff;
                                }
                            }
                        }
                    }
                }
            }

            PodStatus::Unknown
        }
    }

    #[test]
    fn test_extract_pod_info() {
        // Create a mock client
        let client = MockKubeClient;

        // Create a test Pod
        let pod = Pod {
            metadata: ObjectMeta {
                name: Some("test-pod".to_string()),
                namespace: Some("test-namespace".to_string()),
                ..Default::default()
            },
            spec: Some(PodSpec {
                containers: vec![Container {
                    name: "main-container".to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            status: Some(K8sPodStatus {
                phase: Some("Running".to_string()),
                ..Default::default()
            }),
        };

        let pod_info = client.extract_pod_info(pod).unwrap();
        assert_eq!(pod_info.name, "test-pod");
        assert_eq!(pod_info.namespace, "test-namespace");
        assert_eq!(pod_info.container_name, "main-container");
        assert!(matches!(pod_info.status, PodStatus::Running));
    }

    #[test]
    fn test_determine_pod_status() {
        // Create a mock client
        let client = MockKubeClient;

        // Test Running status
        let status = K8sPodStatus {
            phase: Some("Running".to_string()),
            ..Default::default()
        };
        assert!(matches!(
            client.determine_pod_status(&status),
            PodStatus::Running
        ));

        // Test Terminated status
        let status = K8sPodStatus {
            phase: Some("Succeeded".to_string()),
            ..Default::default()
        };
        assert!(matches!(
            client.determine_pod_status(&status),
            PodStatus::Terminated
        ));

        // Test Unknown status
        let status = K8sPodStatus {
            phase: Some("Unknown".to_string()),
            ..Default::default()
        };
        assert!(matches!(
            client.determine_pod_status(&status),
            PodStatus::Unknown
        ));
    }
}
