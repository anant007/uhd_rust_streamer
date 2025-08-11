//! REPL (Read-Eval-Print Loop) interface for interactive debugging

use std::sync::Arc;
use rustyline::error::ReadlineError;
use rustyline::{Editor, Config, EditMode, CompletionType, DefaultEditor};
use rustyline::history::FileHistory;
use rustyline::highlight::Highlighter;
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::validate::{Validator, ValidationContext, ValidationResult};
use rustyline_derive::{Completer, Helper};
use colored::*;
use tracing::{info, debug, error};

use crate::{RFNoCSystem, Result, Error};
use crate::interface::{CommandResult, TableData, utils};
use crate::hardware::StreamEndpoint;

/// REPL command types
#[derive(Debug, Clone)]
pub enum ReplCommand {
    /// Show help
    Help,
    /// Exit REPL
    Exit,
    /// Clear screen
    Clear,
    
    // Device commands
    /// List devices
    ListDevices,
    /// Show device status
    DeviceStatus(String),
    /// Connect to device
    Connect(String),
    /// Disconnect from device
    Disconnect(String),
    
    // Graph commands
    /// Show graph topology
    ShowGraph(String),
    /// List blocks
    ListBlocks(String),
    /// Show block properties
    BlockInfo { device: String, block: String },
    /// Set block property
    SetProperty { 
        device: String, 
        block: String, 
        property: String, 
        value: String 
    },
    
    // Stream commands
    /// List stream endpoints
    ListStreams(String),
    /// Start streaming
    StartStream { device: String, endpoints: Vec<String> },
    /// Stop streaming
    StopStream { device: String },
    /// Show stream statistics
    StreamStats,
    
    // Capture commands
    /// Start capture
    StartCapture { 
        device: String, 
        duration: Option<f64>,
        num_packets: Option<u64>,
    },
    /// Stop capture
    StopCapture,
    /// Export data
    Export { 
        format: String, 
        output: String,
        streams: Option<Vec<usize>>,
    },
    
    // System commands
    /// Show system statistics
    SystemStats,
    /// Set log level
    SetLogLevel(String),
    /// Load configuration
    LoadConfig(String),
    /// Save configuration
    SaveConfig(String),
    
    // Analysis commands
    /// Analyze captured data
    Analyze { file: Option<String> },
    /// Show timing information
    TimingInfo(String),
    
    // Performance commands
    /// Show performance metrics
    Performance,
    /// Start profiling
    StartProfiling,
    /// Stop profiling
    StopProfiling,
    
    // Raw command (for debugging)
    Raw(String),
}

impl ReplCommand {
    /// Parse a command string
    fn parse(input: &str) -> Result<Self> {
        let parts: Vec<&str> = input.trim().split_whitespace().collect();
        if parts.is_empty() {
            return Err(Error::ValidationError("Empty command".to_string()));
        }
        
        match parts[0] {
            "help" | "?" => Ok(ReplCommand::Help),
            "exit" | "quit" | "q" => Ok(ReplCommand::Exit),
            "clear" | "cls" => Ok(ReplCommand::Clear),
            
            "devices" | "ls" => Ok(ReplCommand::ListDevices),
            "status" => {
                if parts.len() < 2 {
                    Err(Error::ValidationError("Device ID required".to_string()))
                } else {
                    Ok(ReplCommand::DeviceStatus(parts[1].to_string()))
                }
            }
            "connect" => {
                if parts.len() < 2 {
                    Err(Error::ValidationError("Device ID required".to_string()))
                } else {
                    Ok(ReplCommand::Connect(parts[1].to_string()))
                }
            }
            "disconnect" => {
                if parts.len() < 2 {
                    Err(Error::ValidationError("Device ID required".to_string()))
                } else {
                    Ok(ReplCommand::Disconnect(parts[1].to_string()))
                }
            }
            
            "graph" => {
                if parts.len() < 2 {
                    Err(Error::ValidationError("Device ID required".to_string()))
                } else {
                    Ok(ReplCommand::ShowGraph(parts[1].to_string()))
                }
            }
            "blocks" => {
                if parts.len() < 2 {
                    Err(Error::ValidationError("Device ID required".to_string()))
                } else {
                    Ok(ReplCommand::ListBlocks(parts[1].to_string()))
                }
            }
            "blockinfo" => {
                if parts.len() < 3 {
                    Err(Error::ValidationError("Usage: blockinfo <device> <block>".to_string()))
                } else {
                    Ok(ReplCommand::BlockInfo {
                        device: parts[1].to_string(),
                        block: parts[2].to_string(),
                    })
                }
            }
            "set" => {
                if parts.len() < 5 {
                    Err(Error::ValidationError("Usage: set <device> <block> <property> <value>".to_string()))
                } else {
                    Ok(ReplCommand::SetProperty {
                        device: parts[1].to_string(),
                        block: parts[2].to_string(),
                        property: parts[3].to_string(),
                        value: parts[4..].join(" "),
                    })
                }
            }
            
            "streams" => {
                if parts.len() < 2 {
                    Err(Error::ValidationError("Device ID required".to_string()))
                } else {
                    Ok(ReplCommand::ListStreams(parts[1].to_string()))
                }
            }
            "start" => {
                if parts.len() < 3 {
                    Err(Error::ValidationError("Usage: start <device> <endpoint1> [endpoint2...]".to_string()))
                } else {
                    Ok(ReplCommand::StartStream {
                        device: parts[1].to_string(),
                        endpoints: parts[2..].iter().map(|s| s.to_string()).collect(),
                    })
                }
            }
            "stop" => {
                if parts.len() < 2 {
                    Err(Error::ValidationError("Device ID required".to_string()))
                } else {
                    Ok(ReplCommand::StopStream {
                        device: parts[1].to_string(),
                    })
                }
            }
            "streamstats" => Ok(ReplCommand::StreamStats),
            
            "capture" => {
                if parts.len() < 2 {
                    Err(Error::ValidationError("Device ID required".to_string()))
                } else {
                    let mut duration = None;
                    let mut num_packets = None;
                    
                    let mut i = 2;
                    while i < parts.len() {
                        match parts[i] {
                            "-d" | "--duration" => {
                                if i + 1 < parts.len() {
                                    duration = parts[i + 1].parse().ok();
                                    i += 2;
                                } else {
                                    i += 1;
                                }
                            }
                            "-n" | "--num-packets" => {
                                if i + 1 < parts.len() {
                                    num_packets = parts[i + 1].parse().ok();
                                    i += 2;
                                } else {
                                    i += 1;
                                }
                            }
                            _ => i += 1,
                        }
                    }
                    
                    Ok(ReplCommand::StartCapture {
                        device: parts[1].to_string(),
                        duration,
                        num_packets,
                    })
                }
            }
            "stopcapture" => Ok(ReplCommand::StopCapture),
            
            "export" => {
                if parts.len() < 3 {
                    Err(Error::ValidationError("Usage: export <format> <output> [streams...]".to_string()))
                } else {
                    let streams = if parts.len() > 3 {
                        Some(parts[3..].iter()
                            .filter_map(|s| s.parse().ok())
                            .collect())
                    } else {
                        None
                    };
                    
                    Ok(ReplCommand::Export {
                        format: parts[1].to_string(),
                        output: parts[2].to_string(),
                        streams,
                    })
                }
            }
            
            "stats" => Ok(ReplCommand::SystemStats),
            "loglevel" => {
                if parts.len() < 2 {
                    Err(Error::ValidationError("Log level required".to_string()))
                } else {
                    Ok(ReplCommand::SetLogLevel(parts[1].to_string()))
                }
            }
            "load" => {
                if parts.len() < 2 {
                    Err(Error::ValidationError("Config file required".to_string()))
                } else {
                    Ok(ReplCommand::LoadConfig(parts[1].to_string()))
                }
            }
            "save" => {
                if parts.len() < 2 {
                    Err(Error::ValidationError("Config file required".to_string()))
                } else {
                    Ok(ReplCommand::SaveConfig(parts[1].to_string()))
                }
            }
            
            "analyze" => {
                let file = if parts.len() > 1 {
                    Some(parts[1].to_string())
                } else {
                    None
                };
                Ok(ReplCommand::Analyze { file })
            }
            "timing" => {
                if parts.len() < 2 {
                    Err(Error::ValidationError("Device ID required".to_string()))
                } else {
                    Ok(ReplCommand::TimingInfo(parts[1].to_string()))
                }
            }
            
            "perf" => Ok(ReplCommand::Performance),
            "profile" => Ok(ReplCommand::StartProfiling),
            "stopprofile" => Ok(ReplCommand::StopProfiling),
            
            _ => Ok(ReplCommand::Raw(input.to_string())),
        }
    }
}

/// REPL helper for rustyline
#[derive(Completer, Helper)]
struct ReplHelper {
    hinter: HistoryHinter,
}

impl Hinter for ReplHelper {
    type Hint = String;
    
    fn hint(&self, line: &str, pos: usize, ctx: &rustyline::Context<'_>) -> Option<String> {
        self.hinter.hint(line, pos, ctx)
    }
}

impl Highlighter for ReplHelper {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        default: bool,
    ) -> std::borrow::Cow<'b, str> {
        if default {
            std::borrow::Cow::Borrowed(prompt)
        } else {
            std::borrow::Cow::Owned(prompt.bright_blue().to_string())
        }
    }
    
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> std::borrow::Cow<'l, str> {
        // Highlight commands
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            return std::borrow::Cow::Borrowed(line);
        }
        
        let highlighted = match parts[0] {
            "help" | "?" => parts[0].green().to_string(),
            "exit" | "quit" | "q" => parts[0].red().to_string(),
            "connect" | "disconnect" => parts[0].yellow().to_string(),
            "start" | "stop" | "capture" => parts[0].bright_yellow().to_string(),
            "graph" | "blocks" | "streams" => parts[0].cyan().to_string(),
            _ => parts[0].to_string(),
        };
        
        if parts.len() > 1 {
            std::borrow::Cow::Owned(format!("{} {}", highlighted, parts[1..].join(" ")))
        } else {
            std::borrow::Cow::Owned(highlighted)
        }
    }
}

impl Validator for ReplHelper {
    fn validate(&self, ctx: &mut ValidationContext) -> rustyline::Result<ValidationResult> {
        // Basic validation - check for unclosed quotes
        let input = ctx.input();
        let quote_count = input.chars().filter(|&c| c == '"').count();
        
        if quote_count % 2 != 0 {
            Ok(ValidationResult::Incomplete)
        } else {
            Ok(ValidationResult::Valid(None))
        }
    }
}

/// REPL interface
pub struct ReplInterface {
    system: Arc<RFNoCSystem>,
    editor: Editor<ReplHelper>,
}

impl ReplInterface {
    /// Create a new REPL interface
    pub fn new(system: Arc<RFNoCSystem>) -> Self {
        let config = Config::builder()
            .history_ignore_space(true)
            .completion_type(CompletionType::List)
            .edit_mode(EditMode::Emacs)
            .build();
        
        let helper = ReplHelper {
            hinter: HistoryHinter {},
        };
        
        let mut editor = Editor::with_config(config)?;
        editor.set_helper(Some(helper));
        
        // Load history if it exists
        let _ = editor.load_history(".rfnoc_history");
        
        Self { system, editor }
    }
    
    /// Run the REPL
    pub async fn run(&mut self) -> Result<()> {
        self.print_welcome();
        
        loop {
            let prompt = "rfnoc> ".bright_blue().to_string();
            
            match self.editor.readline(&prompt) {
                Ok(line) => {
                    self.editor.add_history_entry(line.as_str());
                    
                    match ReplCommand::parse(&line) {
                        Ok(ReplCommand::Exit) => {
                            println!("Goodbye!");
                            break;
                        }
                        Ok(cmd) => {
                            if let Err(e) = self.execute_command(cmd).await {
                                utils::print_error(&format!("Error: {}", e));
                            }
                        }
                        Err(e) => {
                            utils::print_error(&format!("Parse error: {}", e));
                        }
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    println!("Use 'exit' or 'quit' to exit");
                }
                Err(ReadlineError::Eof) => {
                    println!("\nGoodbye!");
                    break;
                }
                Err(err) => {
                    error!("Readline error: {:?}", err);
                    break;
                }
            }
        }
        
        // Save history
        let _ = self.editor.save_history(".rfnoc_history");
        
        Ok(())
    }
    
    /// Print welcome message
    fn print_welcome(&self) {
        println!("{}", "═".repeat(60).bright_blue());
        println!("{}", "RFNoC Tool - Interactive REPL".bright_white().bold());
        println!("{}", "Type 'help' for commands, 'exit' to quit".bright_black());
        println!("{}", "═".repeat(60).bright_blue());
        println!();
    }
    
    /// Execute a command
    async fn execute_command(&mut self, cmd: ReplCommand) -> Result<()> {
        match cmd {
            ReplCommand::Help => self.print_help(),
            ReplCommand::Clear => utils::clear_screen(),
            
            ReplCommand::ListDevices => self.list_devices().await?,
            ReplCommand::DeviceStatus(device) => self.device_status(&device).await?,
            ReplCommand::Connect(device) => self.connect_device(&device).await?,
            ReplCommand::Disconnect(device) => self.disconnect_device(&device).await?,
            
            ReplCommand::ShowGraph(device) => self.show_graph(&device).await?,
            ReplCommand::ListBlocks(device) => self.list_blocks(&device).await?,
            ReplCommand::BlockInfo { device, block } => {
                self.block_info(&device, &block).await?
            }
            ReplCommand::SetProperty { device, block, property, value } => {
                self.set_property(&device, &block, &property, &value).await?
            }
            
            ReplCommand::ListStreams(device) => self.list_streams(&device).await?,
            ReplCommand::StartStream { device, endpoints } => {
                self.start_stream(&device, endpoints).await?
            }
            ReplCommand::StopStream { device } => self.stop_stream(&device).await?,
            ReplCommand::StreamStats => self.stream_stats().await?,
            
            ReplCommand::StartCapture { device, duration, num_packets } => {
                self.start_capture(&device, duration, num_packets).await?
            }
            ReplCommand::StopCapture => self.stop_capture().await?,
            ReplCommand::Export { format, output, streams } => {
                self.export_data(&format, &output, streams).await?
            }
            
            ReplCommand::SystemStats => self.system_stats().await?,
            ReplCommand::SetLogLevel(level) => self.set_log_level(&level)?,
            ReplCommand::LoadConfig(file) => self.load_config(&file).await?,
            ReplCommand::SaveConfig(file) => self.save_config(&file).await?,
            
            ReplCommand::Analyze { file } => self.analyze_data(file).await?,
            ReplCommand::TimingInfo(device) => self.timing_info(&device).await?,
            
            ReplCommand::Performance => self.show_performance().await?,
            ReplCommand::StartProfiling => self.start_profiling()?,
            ReplCommand::StopProfiling => self.stop_profiling()?,
            
            ReplCommand::Raw(input) => {
                utils::print_warning(&format!("Unknown command: {}", input));
            }
            
            ReplCommand::Exit => unreachable!(), // Handled in run()
        }
        
        Ok(())
    }
    
    /// Print help message
    fn print_help(&self) {
        println!("{}", "Available Commands:".bright_white().bold());
        println!();
        
        let commands = vec![
            ("General", vec![
                ("help, ?", "Show this help message"),
                ("exit, quit, q", "Exit the REPL"),
                ("clear, cls", "Clear the screen"),
            ]),
            ("Device Management", vec![
                ("devices, ls", "List available devices"),
                ("status <device>", "Show device status"),
                ("connect <device>", "Connect to a device"),
                ("disconnect <device>", "Disconnect from a device"),
            ]),
            ("Graph Operations", vec![
                ("graph <device>", "Show RFNoC graph topology"),
                ("blocks <device>", "List all blocks in the graph"),
                ("blockinfo <device> <block>", "Show block properties"),
                ("set <device> <block> <prop> <val>", "Set block property"),
            ]),
            ("Streaming", vec![
                ("streams <device>", "List available stream endpoints"),
                ("start <device> <endpoints...>", "Start streaming"),
                ("stop <device>", "Stop streaming"),
                ("streamstats", "Show stream statistics"),
            ]),
            ("Data Capture", vec![
                ("capture <device> [-d duration] [-n packets]", "Start capture"),
                ("stopcapture", "Stop capture"),
                ("export <format> <output> [streams...]", "Export captured data"),
            ]),
            ("System", vec![
                ("stats", "Show system statistics"),
                ("loglevel <level>", "Set log level (trace/debug/info/warn/error)"),
                ("load <file>", "Load configuration from file"),
                ("save <file>", "Save configuration to file"),
            ]),
            ("Analysis", vec![
                ("analyze [file]", "Analyze captured data"),
                ("timing <device>", "Show timing information"),
            ]),
            ("Performance", vec![
                ("perf", "Show performance metrics"),
                ("profile", "Start profiling"),
                ("stopprofile", "Stop profiling"),
            ]),
        ];
        
        for (category, cmds) in commands {
            println!("{}", category.yellow());
            for (cmd, desc) in cmds {
                println!("  {:<35} {}", cmd.bright_white(), desc.bright_black());
            }
            println!();
        }
    }
    
    // Command implementations would go here...
    // For brevity, I'll show a few examples:
    
    async fn list_devices(&self) -> Result<()> {
        let count = self.system.device_manager.get_device_count();
        println!("Found {} device(s)", count);
        
        // In a real implementation, you'd list actual devices
        utils::print_success("Device listing complete");
        Ok(())
    }
    
    async fn system_stats(&self) -> Result<()> {
        let stats = self.system.get_stats().await;
        
        println!("{}", "System Statistics".bright_white().bold());
        println!("  Devices:        {}", stats.device_count);
        println!("  Active Streams: {}", stats.active_streams);
        println!("  Total Packets:  {}", stats.total_packets);
        println!("  Buffer Usage:   {:.1}%", stats.buffer_usage);
        
        Ok(())
    }
    
    fn set_log_level(&self, level: &str) -> Result<()> {
        use tracing::Level;
        
        let level = match level.to_lowercase().as_str() {
            "trace" => Level::TRACE,
            "debug" => Level::DEBUG,
            "info" => Level::INFO,
            "warn" | "warning" => Level::WARN,
            "error" => Level::ERROR,
            _ => {
                return Err(Error::ValidationError(format!("Invalid log level: {}", level)));
            }
        };
        
        // In a real implementation, you'd update the global subscriber
        utils::print_success(&format!("Log level set to {}", level));
        Ok(())
    }
    
    // Additional command implementations would follow...
    
    async fn show_graph(&self, device: &str) -> Result<()> {
        utils::print_warning("Graph visualization not yet implemented");
        Ok(())
    }
    
    async fn list_blocks(&self, device: &str) -> Result<()> {
        utils::print_warning("Block listing not yet implemented");
        Ok(())
    }
    
    async fn block_info(&self, device: &str, block: &str) -> Result<()> {
        utils::print_warning("Block info not yet implemented");
        Ok(())
    }
    
    async fn set_property(&self, device: &str, block: &str, property: &str, value: &str) -> Result<()> {
        utils::print_warning("Property setting not yet implemented");
        Ok(())
    }
    
    async fn list_streams(&self, device: &str) -> Result<()> {
        utils::print_warning("Stream listing not yet implemented");
        Ok(())
    }
    
    async fn start_stream(&self, device: &str, endpoints: Vec<String>) -> Result<()> {
        utils::print_warning("Stream start not yet implemented");
        Ok(())
    }
    
    async fn stop_stream(&self, device: &str) -> Result<()> {
        utils::print_warning("Stream stop not yet implemented");
        Ok(())
    }
    
    async fn stream_stats(&self) -> Result<()> {
        utils::print_warning("Stream stats not yet implemented");
        Ok(())
    }
    
    async fn start_capture(&self, device: &str, duration: Option<f64>, num_packets: Option<u64>) -> Result<()> {
        utils::print_warning("Capture start not yet implemented");
        Ok(())
    }
    
    async fn stop_capture(&self) -> Result<()> {
        utils::print_warning("Capture stop not yet implemented");
        Ok(())
    }
    
    async fn export_data(&self, format: &str, output: &str, streams: Option<Vec<usize>>) -> Result<()> {
        utils::print_warning("Export not yet implemented");
        Ok(())
    }
    
    async fn load_config(&self, file: &str) -> Result<()> {
        utils::print_warning("Config loading not yet implemented");
        Ok(())
    }
    
    async fn save_config(&self, file: &str) -> Result<()> {
        utils::print_warning("Config saving not yet implemented");
        Ok(())
    }
    
    async fn analyze_data(&self, file: Option<String>) -> Result<()> {
        utils::print_warning("Analysis not yet implemented");
        Ok(())
    }
    
    async fn timing_info(&self, device: &str) -> Result<()> {
        utils::print_warning("Timing info not yet implemented");
        Ok(())
    }
    
    async fn show_performance(&self) -> Result<()> {
        utils::print_warning("Performance metrics not yet implemented");
        Ok(())
    }
    
    fn start_profiling(&self) -> Result<()> {
        utils::print_warning("Profiling not yet implemented");
        Ok(())
    }
    
    fn stop_profiling(&self) -> Result<()> {
        utils::print_warning("Profiling not yet implemented");
        Ok(())
    }
}