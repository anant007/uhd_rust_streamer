//! RFNoC Tool - Main entry point

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, error};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use rfnoc_tool::{RFNoCSystem, SystemConfig, Result, Error};
use rfnoc_tool::interface::InterfaceManager;
use rfnoc_tool::data_manager::ExportFormat;

/// RFNoC streaming and analysis tool
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Device arguments (e.g., "addr=192.168.10.2")
    #[arg(short, long, default_value = "addr=192.168.10.2")]
    args: String,
    
    /// Configuration file
    #[arg(short, long)]
    config: Option<PathBuf>,
    
    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info")]
    log_level: String,
    
    /// Output directory for captures
    #[arg(short, long, default_value = "captures")]
    output_dir: PathBuf,
    
    /// Command to execute
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Interactive REPL mode
    Repl {
        /// Enable API server
        #[arg(long)]
        with_api: bool,
        
        /// API server address
        #[arg(long, default_value = "127.0.0.1:8080")]
        api_addr: String,
    },
    
    /// Capture data from device
    Capture {
        /// Number of packets to capture (0 for continuous)
        #[arg(short, long, default_value = "1000")]
        num_packets: u64,
        
        /// Sample rate in Hz
        #[arg(short, long, default_value = "10e6")]
        rate: f64,
        
        /// Center frequency in Hz
        #[arg(short, long, default_value = "100e6")]
        freq: f64,
        
        /// Gain in dB
        #[arg(short, long, default_value = "30")]
        gain: f64,
        
        /// Output file prefix
        #[arg(short, long, default_value = "capture")]
        file: String,
        
        /// Enable multi-stream capture
        #[arg(long)]
        multi_stream: bool,
        
        /// Use PPS reset for synchronization
        #[arg(long)]
        pps_reset: bool,
        
        /// Enable analysis after capture
        #[arg(long)]
        analyze: bool,
        
        /// CSV output file for analysis
        #[arg(long)]
        csv: Option<PathBuf>,
    },
    
    /// Analyze existing capture file
    Analyze {
        /// Input file to analyze
        file: PathBuf,
        
        /// Output CSV file
        #[arg(short, long)]
        output: Option<PathBuf>,
        
        /// Tick rate (if not in file header)
        #[arg(long)]
        tick_rate: Option<f64>,
    },
    
    /// Export data to different formats
    Export {
        /// Input file(s)
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
        
        /// Output format (csv, matlab, hdf5)
        #[arg(short, long, default_value = "csv")]
        format: String,
        
        /// Output file
        #[arg(short, long)]
        output: PathBuf,
        
        /// Specific streams to export
        #[arg(short, long)]
        streams: Option<Vec<usize>>,
    },
    
    /// Show device information
    Info {
        /// Show detailed graph topology
        #[arg(long)]
        graph: bool,
        
        /// Show available stream endpoints
        #[arg(long)]
        streams: bool,
        
        /// Output YAML template
        #[arg(long)]
        yaml_template: Option<PathBuf>,
    },
    
    /// Run performance benchmark
    Benchmark {
        /// Duration in seconds
        #[arg(short, long, default_value = "10")]
        duration: f64,
        
        /// Sample rate to test
        #[arg(short, long, default_value = "50e6")]
        rate: f64,
        
        /// Number of streams
        #[arg(short, long, default_value = "1")]
        streams: usize,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    // Initialize logging
    init_logging(&args.log_level)?;
    
    // Load or create configuration
    let mut config = if let Some(config_path) = &args.config {
        info!("Loading configuration from: {:?}", config_path);
        SystemConfig::from_file(config_path)?
    } else {
        info!("Using default configuration");
        SystemConfig::default()
    };
    
    // Override with command line arguments
    config.device.args = args.args.clone();
    config.stream.output_directory = args.output_dir.clone();
    
    // Create the system
    let system = Arc::new(RFNoCSystem::new(config).await?);
    
    // Start the system
    system.start().await?;
    
    // Handle the command
    let result = match args.command {
        Some(Command::Repl { with_api, api_addr }) => {
            run_repl(system, with_api, &api_addr).await
        }
        Some(Command::Capture { 
            num_packets, rate, freq, gain, file, 
            multi_stream, pps_reset, analyze, csv 
        }) => {
            run_capture(
                system, num_packets, rate, freq, gain, 
                &file, multi_stream, pps_reset, analyze, csv
            ).await
        }
        Some(Command::Analyze { file, output, tick_rate }) => {
            run_analyze(&file, output, tick_rate).await
        }
        Some(Command::Export { inputs, format, output, streams }) => {
            run_export(system, inputs, &format, output, streams).await
        }
        Some(Command::Info { graph, streams, yaml_template }) => {
            run_info(system, graph, streams, yaml_template).await
        }
        Some(Command::Benchmark { duration, rate, streams }) => {
            run_benchmark(system, duration, rate, streams).await
        }
        None => {
            // Default to REPL if no command specified
            run_repl(system, false, "").await
        }
    };
    
    // Handle any errors
    if let Err(e) = result {
        error!("Error: {}", e);
        std::process::exit(1);
    }
    
    Ok(())
}

/// Initialize logging system
fn init_logging(level: &str) -> Result<()> {
    let filter = EnvFilter::try_new(level)
        .map_err(|e| Error::ConfigError(format!("Invalid log level: {}", e)))?;
    
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();
    
    Ok(())
}

/// Run REPL mode
async fn run_repl(
    system: Arc<RFNoCSystem>, 
    with_api: bool, 
    api_addr: &str
) -> Result<()> {
    info!("Starting RFNoC tool in REPL mode");
    
    let mut interface = InterfaceManager::new(system)?;
    
    // Start monitoring
    interface.start_monitoring().await?;
    
    // Start API server if requested
    if with_api {
        interface.start_api(api_addr).await?;
        info!("API server started on {}", api_addr);
    }
    
    // Start REPL
    interface.start_repl().await?;
    
    // Run the REPL (blocking)
    interface.run_repl().await?;
    
    // Clean shutdown
    interface.stop().await?;
    
    Ok(())
}

/// Run capture mode
async fn run_capture(
    system: Arc<RFNoCSystem>,
    num_packets: u64,
    rate: f64,
    freq: f64,
    gain: f64,
    file_prefix: &str,
    multi_stream: bool,
    pps_reset: bool,
    analyze: bool,
    csv: Option<PathBuf>,
) -> Result<()> {
    info!("Starting capture mode");
    info!("  Packets: {}", if num_packets == 0 { "continuous".to_string() } else { num_packets.to_string() });
    info!("  Rate: {:.2} MHz", rate / 1e6);
    info!("  Frequency: {:.2} MHz", freq / 1e6);
    info!("  Gain: {} dB", gain);
    info!("  Multi-stream: {}", multi_stream);
    info!("  PPS reset: {}", pps_reset);
    
    // Initialize device and graph
    let device_id = "usrp_0"; // Would be determined from device discovery
    let graph = system.graph_processor.initialize_graph(device_id.to_string()).await?;
    
    // Configure radio parameters
    system.graph_processor.set_block_property(
        device_id, "0/Radio#0", "freq", &freq.to_string()
    ).await?;
    system.graph_processor.set_block_property(
        device_id, "0/Radio#0", "gain", &gain.to_string()
    ).await?;
    system.graph_processor.set_block_property(
        device_id, "0/Radio#0", "rate", &rate.to_string()
    ).await?;
    
    // Get stream endpoints
    let endpoints = system.graph_processor.get_stream_endpoints(device_id)?;
    
    // Filter endpoints based on multi-stream setting
    let endpoints = if multi_stream {
        endpoints
    } else {
        endpoints.into_iter().take(1).collect()
    };
    
    info!("Using {} stream endpoint(s)", endpoints.len());
    
    // Start streaming
    let handles = system.graph_processor.start_streaming(device_id, endpoints).await?;
    
    // Set up signal handler for graceful shutdown
    let stop_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_flag_clone = stop_flag.clone();
    
    ctrlc::set_handler(move || {
        info!("Received Ctrl+C, stopping capture...");
        stop_flag_clone.store(true, std::sync::atomic::Ordering::Relaxed);
    }).map_err(|e| Error::SystemError(format!("Failed to set signal handler: {}", e)))?;
    
    // Capture loop
    if num_packets == 0 {
        // Continuous capture
        info!("Starting continuous capture. Press Ctrl+C to stop.");
        
        while !stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            
            // Print periodic statistics
            let stats = system.get_stats().await;
            info!("Packets: {}, Buffer: {:.1}%", stats.total_packets, stats.buffer_usage);
        }
    } else {
        // Fixed number of packets
        let progress = indicatif::ProgressBar::new(num_packets);
        progress.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} packets")
                .unwrap()
                .progress_chars("#>-")
        );
        
        let start_time = std::time::Instant::now();
        let target_packets = num_packets * handles.len() as u64;
        
        while system.get_stats().await.total_packets < target_packets {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            progress.set_position(system.get_stats().await.total_packets);
            
            if stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
                info!("Capture interrupted by user");
                break;
            }
        }
        
        progress.finish();
    }
    
    // Stop streaming
    system.graph_processor.stop_streaming(handles).await?;
    
    let stats = system.get_stats().await;
    let duration = std::time::Instant::now().duration_since(start_time).as_secs_f64();
    
    info!("Capture complete:");
    info!("  Total packets: {}", stats.total_packets);
    info!("  Duration: {:.2} seconds", duration);
    info!("  Average rate: {:.2} Msps", (stats.total_packets as f64 / duration) / 1e6);
    
    // Run analysis if requested
    if analyze {
        info!("Running analysis...");
        
        let csv_file = csv.unwrap_or_else(|| {
            PathBuf::from(format!("{}_analysis.csv", file_prefix))
        });
        
        // Export to CSV for analysis
        system.data_manager.request_export(
            vec![0], // All streams
            ExportFormat::Csv,
            csv_file.clone(),
        ).await?;
        
        info!("Analysis saved to: {:?}", csv_file);
    }
    
    // Stop the system
    system.stop().await?;
    
    Ok(())
}

/// Run analysis mode
async fn run_analyze(
    file: &PathBuf,
    output: Option<PathBuf>,
    tick_rate: Option<f64>,
) -> Result<()> {
    info!("Analyzing file: {:?}", file);
    
    // This would implement the analysis functionality
    // For now, just a placeholder
    
    error!("Analysis mode not yet implemented");
    Err(Error::SystemError("Not implemented".to_string()))
}

/// Run export mode
async fn run_export(
    system: Arc<RFNoCSystem>,
    inputs: Vec<PathBuf>,
    format: &str,
    output: PathBuf,
    streams: Option<Vec<usize>>,
) -> Result<()> {
    info!("Exporting {} files to {:?}", inputs.len(), output);
    
    let export_format = match format {
        "csv" => ExportFormat::Csv,
        "matlab" | "mat" => ExportFormat::Matlab,
        "hdf5" | "h5" => ExportFormat::Hdf5,
        _ => return Err(Error::ValidationError(format!("Unknown format: {}", format))),
    };
    
    // Request export
    let streams = streams.unwrap_or_else(|| vec![]);
    system.data_manager.request_export(streams, export_format, output).await?;
    
    info!("Export complete");
    Ok(())
}

/// Run info mode
async fn run_info(
    system: Arc<RFNoCSystem>,
    show_graph: bool,
    show_streams: bool,
    yaml_template: Option<PathBuf>,
) -> Result<()> {
    info!("Device information:");
    
    // Initialize device
    let device_id = "usrp_0";
    let graph = system.graph_processor.initialize_graph(device_id.to_string()).await?;
    
    // Get device info
    let device_count = system.device_manager.get_device_count();
    println!("Connected devices: {}", device_count);
    
    if show_graph {
        println!("\nGraph topology:");
        let topology = system.graph_processor.get_topology(device_id)?;
        println!("  Blocks: {}", topology.blocks.len());
        for block in &topology.blocks {
            println!("    {} ({})", block.block_id, block.block_type);
            println!("      Inputs: {}, Outputs: {}", 
                block.num_input_ports, block.num_output_ports);
        }
    }
    
    if show_streams {
        println!("\nStream endpoints:");
        let endpoints = system.graph_processor.get_stream_endpoints(device_id)?;
        for (i, endpoint) in endpoints.iter().enumerate() {
            println!("  Stream {}: {}:{}", i, endpoint.block_id, endpoint.port);
        }
    }
    
    if let Some(output) = yaml_template {
        info!("Generating YAML template: {:?}", output);
        // Would generate YAML template here
    }
    
    system.stop().await?;
    Ok(())
}

/// Run benchmark mode
async fn run_benchmark(
    system: Arc<RFNoCSystem>,
    duration: f64,
    rate: f64,
    num_streams: usize,
) -> Result<()> {
    info!("Running benchmark:");
    info!("  Duration: {} seconds", duration);
    info!("  Sample rate: {:.2} MHz", rate / 1e6);
    info!("  Streams: {}", num_streams);
    
    // Initialize device
    let device_id = "usrp_0";
    let graph = system.graph_processor.initialize_graph(device_id.to_string()).await?;
    
    // Configure for benchmark
    system.graph_processor.set_block_property(
        device_id, "0/Radio#0", "rate", &rate.to_string()
    ).await?;
    
    // Get endpoints
    let endpoints = system.graph_processor.get_stream_endpoints(device_id)?;
    let endpoints: Vec<_> = endpoints.into_iter().take(num_streams).collect();
    
    if endpoints.len() < num_streams {
        return Err(Error::ValidationError(format!(
            "Requested {} streams but only {} available", 
            num_streams, endpoints.len()
        )));
    }
    
    // Start streaming
    let handles = system.graph_processor.start_streaming(device_id, endpoints).await?;
    
    // Run benchmark
    let start_time = std::time::Instant::now();
    let mut last_packets = 0u64;
    let mut samples_per_second = Vec::new();
    
    while start_time.elapsed().as_secs_f64() < duration {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        
        let stats = system.get_stats().await;
        let packets_this_second = stats.total_packets - last_packets;
        last_packets = stats.total_packets;
        
        // Assuming default samples per packet
        let samples_this_second = packets_this_second * 2000; // Default SPB
        samples_per_second.push(samples_this_second as f64);
        
        println!("Rate: {:.2} Msps, Buffer: {:.1}%", 
            samples_this_second as f64 / 1e6, stats.buffer_usage);
    }
    
    // Stop streaming
    system.graph_processor.stop_streaming(handles).await?;
    
    // Calculate statistics
    let avg_rate: f64 = samples_per_second.iter().sum::<f64>() / samples_per_second.len() as f64;
    let min_rate = samples_per_second.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_rate = samples_per_second.iter().cloned().fold(0.0, f64::max);
    
    println!("\nBenchmark Results:");
    println!("  Average rate: {:.2} Msps ({:.1}% of requested)", 
        avg_rate / 1e6, (avg_rate / rate) * 100.0);
    println!("  Min rate: {:.2} Msps", min_rate / 1e6);
    println!("  Max rate: {:.2} Msps", max_rate / 1e6);
    
    let stats = system.get_stats().await;
    println!("  Total packets: {}", stats.total_packets);
    println!("  Buffer overflows: {}", 
        system.data_manager.get_statistics().buffer_overflows);
    
    system.stop().await?;
    Ok(())
}