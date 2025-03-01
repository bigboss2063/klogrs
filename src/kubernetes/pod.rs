use std::fmt;

/// Pod status
#[derive(Debug, Clone, PartialEq)]
pub enum PodStatus {
    Running,
    Pending,
    CrashLoopBackOff,
    Terminated,
    Unknown,
}

impl fmt::Display for PodStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PodStatus::Running => write!(f, "Running"),
            PodStatus::Pending => write!(f, "Pending"),
            PodStatus::CrashLoopBackOff => write!(f, "CrashLoopBackOff"),
            PodStatus::Terminated => write!(f, "Terminated"),
            PodStatus::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Pod information
#[derive(Debug, Clone)]
pub struct PodInfo {
    /// Pod name
    pub name: String,
    /// Pod namespace
    pub namespace: String,
    /// Pod status
    pub status: PodStatus,
    /// Container name
    pub container_name: String,
}

impl PodInfo {
    /// Check if the pod is in a state where logs can be retrieved
    pub fn can_get_logs(&self) -> bool {
        match self.status {
            PodStatus::Running | PodStatus::CrashLoopBackOff => true,
            _ => false,
        }
    }

    /// Get a short name for the pod (first 8 characters)
    pub fn short_name(&self) -> String {
        if self.name.len() <= 8 {
            self.name.clone()
        } else {
            self.name[..8].to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pod_status_display() {
        assert_eq!(format!("{}", PodStatus::Running), "Running");
        assert_eq!(format!("{}", PodStatus::Pending), "Pending");
        assert_eq!(
            format!("{}", PodStatus::CrashLoopBackOff),
            "CrashLoopBackOff"
        );
        assert_eq!(format!("{}", PodStatus::Terminated), "Terminated");
        assert_eq!(format!("{}", PodStatus::Unknown), "Unknown");
    }

    #[test]
    fn test_can_get_logs() {
        let running_pod = PodInfo {
            name: "test-pod".to_string(),
            namespace: "default".to_string(),
            status: PodStatus::Running,
            container_name: "main".to_string(),
        };
        assert!(running_pod.can_get_logs());

        let crash_pod = PodInfo {
            name: "test-pod".to_string(),
            namespace: "default".to_string(),
            status: PodStatus::CrashLoopBackOff,
            container_name: "main".to_string(),
        };
        assert!(crash_pod.can_get_logs());

        let terminated_pod = PodInfo {
            name: "test-pod".to_string(),
            namespace: "default".to_string(),
            status: PodStatus::Terminated,
            container_name: "main".to_string(),
        };
        assert!(!terminated_pod.can_get_logs());
    }

    #[test]
    fn test_short_name() {
        let short_pod = PodInfo {
            name: "short".to_string(),
            namespace: "default".to_string(),
            status: PodStatus::Running,
            container_name: "main".to_string(),
        };
        assert_eq!(short_pod.short_name(), "short");

        let long_pod = PodInfo {
            name: "very-long-pod-name".to_string(),
            namespace: "default".to_string(),
            status: PodStatus::Running,
            container_name: "main".to_string(),
        };
        assert_eq!(long_pod.short_name(), "very-lon");
    }
}
