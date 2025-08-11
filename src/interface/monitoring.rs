use std::sync::Arc;
use crate::{RFNoCSystem, Result};

pub struct MonitoringService {
    system: Arc<RFNoCSystem>,
}

impl MonitoringService {
    pub fn new(system: Arc<RFNoCSystem>) -> Result<Self> {
        Ok(Self { system })
    }
    
    pub async fn start(&self) -> Result<()> {
        Ok(())
    }
    
    pub async fn stop(&self) -> Result<()> {
        Ok(())
    }
    
    pub async fn get_metrics(&self) -> SystemMetrics {
        SystemMetrics::default()
    }
}

#[derive(Debug, Default)]
pub struct SystemMetrics;