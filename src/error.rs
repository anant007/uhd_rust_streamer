//! Error types and handling for the RFNoC tool

use thiserror::Error;

/// Main error type for the RFNoC tool
#[derive(Debug, Error)]
pub enum Error {
    /// Hardware device error
    #[error("Hardware error: {0}")]
    HardwareError(String),
    
    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    /// Data processing error
    #[error("Processing error: {0}")]
    ProcessingError(String),
    
    /// I/O error
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    
    /// Channel communication error
    #[error("Channel error: {0}")]
    ChannelError(String),
    
    /// System-level error
    #[error("System error: {0}")]
    SystemError(String),
    
    /// UHD API error
    #[error("UHD error: {0}")]
    UhdError(String),
    
    /// JSON error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    
    /// Timeout error
    #[error("Operation timed out: {0}")]
    TimeoutError(String),
    
    /// Validation error
    #[error("Validation error: {0}")]
    ValidationError(String),
    
    /// Export error
    #[error("Export error: {0}")]
    ExportError(String),
    
    /// YAML parsing error
    #[error("YAML error: {0}")]
    YamlError(#[from] serde_yaml::Error),
    
    /// CSV error
    #[error("CSV error: {0}")]
    CsvError(#[from] csv::Error),
    
    /// HDF5 error
    #[error("HDF5 error: {0}")]
    Hdf5Error(#[from] hdf5::Error),
    
    /// Other errors
    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// Result type alias using our Error type
pub type Result<T> = std::result::Result<T, Error>;

/// Extension trait for converting channel errors
pub trait ChannelErrorExt<T> {
    /// Convert channel send/recv errors to our error type
    fn map_channel_err(self) -> Result<T>;
}

impl<T> ChannelErrorExt<T> for std::result::Result<T, async_channel::SendError<T>> {
    fn map_channel_err(self) -> Result<T> {
        self.map_err(|_| Error::ChannelError("Channel send failed".to_string()))
    }
}

impl<T> ChannelErrorExt<T> for std::result::Result<T, async_channel::RecvError> {
    fn map_channel_err(self) -> Result<T> {
        self.map_err(|_| Error::ChannelError("Channel receive failed".to_string()))
    }
}

impl<T, E> ChannelErrorExt<T> for std::result::Result<T, crossbeam::channel::SendError<E>> {
    fn map_channel_err(self) -> Result<T> {
        self.map_err(|_| Error::ChannelError("Crossbeam channel send failed".to_string()))
    }
}

impl<T> ChannelErrorExt<T> for std::result::Result<T, crossbeam::channel::RecvError> {
    fn map_channel_err(self) -> Result<T> {
        self.map_err(|_| Error::ChannelError("Crossbeam channel receive failed".to_string()))
    }
}