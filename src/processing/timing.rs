//! Timing and PPS synchronization

use std::sync::Arc;
use parking_lot::RwLock;
use tracing::info;

use crate::{Result, Error};
use crate::config::SystemConfig;
use crate::hardware::{RfnocGraph, TimeSpec};

/// PPS synchronization status
#[derive(Debug, Clone)]
pub struct PpsSync {
    pub synchronized: bool,
    pub last_pps_time: Option<TimeSpec>,
}

/// Timing manager
pub struct TimingManager {
    config: Arc<RwLock<SystemConfig>>,
}

impl TimingManager {
    /// Create a new timing manager
    pub fn new(config: Arc<RwLock<SystemConfig>>) -> Self {
        Self { config }
    }
    
    /// Start timing manager
    pub async fn start(&self) -> Result<()> {
        Ok(())
    }
    
    /// Stop timing manager
    pub async fn stop(&self) -> Result<()> {
        Ok(())
    }
    
    /// Perform PPS reset
    pub async fn perform_pps_reset(&self, graph: &mut RfnocGraph) -> Result<()> {
        info!("Performing PPS reset");
        
        let config = self.config.read();
        if !config.device.pps_reset.enable {
            return Ok(());
        }
        
        // Set time to 0 at next PPS
        graph.set_time_next_pps(TimeSpec::new(0, 0))?;
        
        // Wait for PPS
        tokio::time::sleep(tokio::time::Duration::from_secs_f64(
            config.device.pps_reset.wait_time_sec
        )).await;
        
        // Verify
        let current_time = graph.get_time_now();
        info!("Current time after PPS reset: {:?}", current_time);
        
        Ok(())
    }
}