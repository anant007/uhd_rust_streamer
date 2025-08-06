//! Configuration validation utilities

use crate::{Result, Error};
use super::SystemConfig;

/// Validate system configuration
pub fn validate_config(config: &SystemConfig) -> Result<()> {
    // Validate device args
    if config.device.args.is_empty() {
        return Err(Error::ValidationError("Device args cannot be empty".to_string()));
    }
    
    // Validate tick rate
    if config.stream.tick_rate <= 0.0 {
        return Err(Error::ValidationError("Tick rate must be positive".to_string()));
    }
    
    // Validate buffer sizes
    if config.stream.multi_stream.buffer_config.ring_buffer_size < 1024 {
        return Err(Error::ValidationError("Ring buffer size too small".to_string()));
    }
    
    // Validate PPS reset config
    if config.device.pps_reset.enable {
        if config.device.pps_reset.wait_time_sec <= 0.0 {
            return Err(Error::ValidationError("PPS wait time must be positive".to_string()));
        }
    }
    
    Ok(())
}