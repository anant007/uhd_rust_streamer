//! User interface layer with REPL, API, and monitoring

pub mod repl;
pub mod api;
pub mod monitoring;

use std::sync::Arc;
use parking_lot::RwLock;
use tracing::{info, debug};

use crate::{RFNoCSystem, Result, Error};
use crate::config::SystemConfig;

pub use repl::{ReplInterface, ReplCommand};
pub use api::{ApiServer, ApiHandler};
pub use monitoring::{MonitoringService, SystemMetrics};

/// Main interface coordinator
pub struct InterfaceManager {
    /// System reference
    system: Arc<RFNoCSystem>,
    /// REPL interface
    repl: Option<ReplInterface>,
    /// API server
    api_server: Option<ApiServer>,
    /// Monitoring service
    monitoring: Arc<MonitoringService>,
}

impl InterfaceManager {
    /// Create a new interface manager
    pub fn new(system: Arc<RFNoCSystem>) -> Result<Self> {
        let monitoring = Arc::new(MonitoringService::new(system.clone())?);
        
        Ok(Self {
            system,
            repl: None,
            api_server: None,
            monitoring,
        })
    }
    
    /// Start the REPL interface
    pub async fn start_repl(&mut self) -> Result<()> {
        info!("Starting REPL interface");
        
        let repl = ReplInterface::new(self.system.clone());
        self.repl = Some(repl);
        
        Ok(())
    }
    
    /// Start the API server
    pub async fn start_api(&mut self, addr: &str) -> Result<()> {
        info!("Starting API server on {}", addr);
        
        let api_server = ApiServer::new(self.system.clone(), addr).await?;
        self.api_server = Some(api_server);
        
        Ok(())
    }
    
    /// Start monitoring service
    pub async fn start_monitoring(&self) -> Result<()> {
        info!("Starting monitoring service");
        self.monitoring.start().await
    }
    
    /// Run the REPL (blocking)
    pub async fn run_repl(&mut self) -> Result<()> {
        if let Some(repl) = &mut self.repl {
            repl.run().await
        } else {
            Err(Error::SystemError("REPL not started".to_string()))
        }
    }
    
    /// Stop all interfaces
    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping interfaces");
        
        if let Some(api_server) = &mut self.api_server {
            api_server.stop().await?;
        }
        
        self.monitoring.stop().await?;
        
        Ok(())
    }
    
    /// Get system metrics
    pub async fn get_metrics(&self) -> SystemMetrics {
        self.monitoring.get_metrics().await
    }
}

/// Command result for display
#[derive(Debug)]
pub enum CommandResult {
    /// Success with optional message
    Success(Option<String>),
    /// Error with message
    Error(String),
    /// Table data for display
    Table(TableData),
    /// JSON data
    Json(serde_json::Value),
    /// Progress indicator
    Progress(ProgressInfo),
}

/// Table data for structured display
#[derive(Debug)]
pub struct TableData {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// Progress information
#[derive(Debug)]
pub struct ProgressInfo {
    pub message: String,
    pub current: u64,
    pub total: Option<u64>,
}

/// Interface utilities
pub mod utils {
    use colored::*;
    use console::Term;
    use indicatif::{ProgressBar, ProgressStyle};
    
    /// Print a success message
    pub fn print_success(msg: &str) {
        println!("{} {}", "✓".green(), msg);
    }
    
    /// Print an error message
    pub fn print_error(msg: &str) {
        eprintln!("{} {}", "✗".red(), msg);
    }
    
    /// Print a warning message
    pub fn print_warning(msg: &str) {
        println!("{} {}", "⚠".yellow(), msg);
    }
    
    /// Print a table
    pub fn print_table(headers: &[String], rows: &[Vec<String>]) {
        use prettytable::{Table, Row, Cell};
        
        let mut table = Table::new();
        
        // Add headers
        let header_cells: Vec<Cell> = headers.iter()
            .map(|h| Cell::new(h).style_spec("Fb"))
            .collect();
        table.add_row(Row::new(header_cells));
        
        // Add rows
        for row in rows {
            let cells: Vec<Cell> = row.iter()
                .map(|c| Cell::new(c))
                .collect();
            table.add_row(Row::new(cells));
        }
        
        table.printstd();
    }
    
    /// Create a progress bar
    pub fn create_progress_bar(total: u64, message: &str) -> ProgressBar {
        let pb = ProgressBar::new(total);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("#>-")
        );
        pb.set_message(message.to_string());
        pb
    }
    
    /// Clear the terminal
    pub fn clear_screen() {
        let term = Term::stdout();
        let _ = term.clear_screen();
    }
    
    /// Format bytes for display
    pub fn format_bytes(bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        
        if bytes == 0 {
            return "0 B".to_string();
        }
        
        let mut size = bytes as f64;
        let mut unit_index = 0;
        
        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }
        
        if unit_index == 0 {
            format!("{} {}", bytes, UNITS[unit_index])
        } else {
            format!("{:.2} {}", size, UNITS[unit_index])
        }
    }
    
    /// Format duration for display
    pub fn format_duration(secs: f64) -> String {
        if secs < 1.0 {
            format!("{:.1} ms", secs * 1000.0)
        } else if secs < 60.0 {
            format!("{:.1} s", secs)
        } else if secs < 3600.0 {
            let mins = (secs / 60.0) as u64;
            let secs = secs % 60.0;
            format!("{} min {:.1} s", mins, secs)
        } else {
            let hours = (secs / 3600.0) as u64;
            let mins = ((secs % 3600.0) / 60.0) as u64;
            format!("{} hr {} min", hours, mins)
        }
    }
    
    /// Format sample rate for display
    pub fn format_sample_rate(rate: f64) -> String {
        if rate >= 1e9 {
            format!("{:.2} Gsps", rate / 1e9)
        } else if rate >= 1e6 {
            format!("{:.2} Msps", rate / 1e6)
        } else if rate >= 1e3 {
            format!("{:.2} Ksps", rate / 1e3)
        } else {
            format!("{:.2} sps", rate)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::utils::*;
    
    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
        assert_eq!(format_bytes(1073741824), "1.00 GB");
    }
    
    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0.5), "500.0 ms");
        assert_eq!(format_duration(1.5), "1.5 s");
        assert_eq!(format_duration(65.0), "1 min 5.0 s");
        assert_eq!(format_duration(3665.0), "1 hr 1 min");
    }
    
    #[test]
    fn test_format_sample_rate() {
        assert_eq!(format_sample_rate(100.0), "100.00 sps");
        assert_eq!(format_sample_rate(1000.0), "1.00 Ksps");
        assert_eq!(format_sample_rate(1e6), "1.00 Msps");
        assert_eq!(format_sample_rate(2.5e9), "2.50 Gsps");
    }
}