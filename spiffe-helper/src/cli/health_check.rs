use serde::{Deserialize, Serialize};

const DEFAULT_LIVENESS_PATH: &str = "/health/live";
const DEFAULT_READINESS_PATH: &str = "/health/ready";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthChecksConfig {
    pub listener_enabled: bool,
    pub bind_port: u16,
    pub liveness_path: Option<String>,
    pub readiness_path: Option<String>,
}

impl HealthChecksConfig {
    #[must_use]
    pub fn bind_addr(&self) -> String {
        format!("0.0.0.0:{}", self.bind_port)
    }

    #[must_use]
    pub fn liveness_path(&self) -> String {
        self.liveness_path
            .clone()
            .unwrap_or_else(|| DEFAULT_LIVENESS_PATH.to_string())
    }

    #[must_use]
    pub fn readiness_path(&self) -> String {
        self.readiness_path
            .clone()
            .unwrap_or_else(|| DEFAULT_READINESS_PATH.to_string())
    }
}
