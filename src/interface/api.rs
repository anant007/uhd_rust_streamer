use std::sync::Arc;
use crate::{RFNoCSystem, Result};

pub struct ApiServer {
    system: Arc<RFNoCSystem>,
}

impl ApiServer {
    pub async fn new(system: Arc<RFNoCSystem>, _addr: &str) -> Result<Self> {
        Ok(Self { system })
    }
    
    pub async fn stop(&mut self) -> Result<()> {
        Ok(())
    }
}

pub struct ApiHandler;