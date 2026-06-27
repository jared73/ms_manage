use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRecord {
    pub id: String,
    pub timestamp: String,
    pub level: String,
    pub source: String,
    pub message: String,
    #[serde(default)]
    pub detail: Option<String>,
}

pub type Logs = Vec<LogRecord>;
