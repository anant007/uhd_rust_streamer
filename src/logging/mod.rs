//! Logging and telemetry infrastructure

use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use crate::{Result, Error};

/// Initialize the logging system
pub fn init_logging(level: &str) -> Result<()> {
    let filter = EnvFilter::try_new(level)
        .map_err(|e| Error::ConfigError(format!("Invalid log level: {}", e)))?;
    
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();
    
    Ok(())
}

/// Logging configuration
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    pub level: String,
    pub file: Option<String>,
    pub format: LogFormat,
}

#[derive(Debug, Clone)]
pub enum LogFormat {
    Pretty,
    Json,
    Compact,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file: None,
            format: LogFormat::Pretty,
        }
    }
}