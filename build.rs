use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=src/hardware/uhd_wrapper.h");
    println!("cargo:rerun-if-changed=src/hardware/uhd_wrapper.cpp");
    println!("cargo:rerun-if-changed=src/hardware/uhd_bindings.rs");

    // Get UHD paths using pkg-config
    let uhd_prefix = get_uhd_prefix();
    let uhd_include = uhd_prefix.join("include");
    let uhd_lib = uhd_prefix.join("lib");

    // Get Boost include path
    let boost_include = get_boost_include_path();

    // Build the cxx bridge
    let mut bridge = cxx_build::bridge("src/hardware/uhd_bindings.rs");
    
    // CRITICAL: Add the project root to the include path so it can find src/hardware/uhd_wrapper.h
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    bridge.include(&manifest_dir);
    
    // Add other include directories
    bridge.include(&uhd_include);
    bridge.include("/usr/local/include");
    bridge.include("/usr/include");
    
    if let Some(boost_path) = boost_include {
        bridge.include(boost_path);
    }
    
    // Add the actual source file
    bridge.file("src/hardware/uhd_wrapper.cpp");
    
    // Set C++ standard and optimization flags
    bridge.flag("-std=c++14");
    bridge.flag("-O3");
    bridge.flag("-march=native");
    
    // Set defines if needed
    bridge.define("BOOST_ALL_DYN_LINK", None);
    bridge.define("BOOST_ALL_NO_LIB", None);
    
    // Compile the bridge
    bridge.compile("uhd_wrapper");

    // Link libraries
    println!("cargo:rustc-link-search=native={}", uhd_lib.display());
    println!("cargo:rustc-link-lib=dylib=uhd");
    println!("cargo:rustc-link-lib=dylib=boost_system");
    println!("cargo:rustc-link-lib=dylib=boost_thread");
    println!("cargo:rustc-link-lib=dylib=boost_program_options");
    println!("cargo:rustc-link-lib=dylib=stdc++");
    println!("cargo:rustc-link-lib=dylib=pthread");
}

fn get_uhd_prefix() -> PathBuf {
    let output = Command::new("pkg-config")
        .args(["--variable=prefix", "uhd"])
        .output()
        .expect("Failed to run pkg-config for UHD");
    
    if !output.status.success() {
        panic!("pkg-config failed for UHD. Make sure UHD is installed and pkg-config can find it.");
    }
    
    let prefix = String::from_utf8(output.stdout)
        .expect("Invalid UTF-8 in pkg-config output")
        .trim()
        .to_string();
    
    if prefix.is_empty() {
        // Fallback to default location
        PathBuf::from("/usr/local")
    } else {
        PathBuf::from(prefix)
    }
}

fn get_boost_include_path() -> Option<PathBuf> {
    // Try to find Boost using pkg-config first
    if let Ok(output) = Command::new("pkg-config")
        .args(["--cflags", "boost"])
        .output() 
    {
        if output.status.success() {
            let flags = String::from_utf8_lossy(&output.stdout);
            for flag in flags.split_whitespace() {
                if let Some(path) = flag.strip_prefix("-I") {
                    return Some(PathBuf::from(path));
                }
            }
        }
    }
    
    // Check common locations
    let common_paths = [
        "/usr/include/boost",
        "/usr/local/include/boost",
        "/opt/homebrew/include",
    ];
    
    for path in &common_paths {
        let boost_path = PathBuf::from(path);
        if boost_path.exists() {
            return Some(boost_path.parent().unwrap().to_path_buf());
        }
    }
    
    None
}