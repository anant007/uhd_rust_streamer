use std::env;
use std::path::PathBuf;

fn main() {
    // Find UHD installation
    let uhd_dir = find_uhd_dir();
    
    // Print cargo directives
    println!("cargo:rerun-if-changed=src/hardware/uhd_wrapper.h");
    println!("cargo:rerun-if-changed=src/hardware/uhd_wrapper.cpp");
    
    // Set up include paths
    let uhd_include = uhd_dir.join("include");
    let boost_include = find_boost_include();
    
    // Build the C++ wrapper
    cxx_build::bridge("src/hardware/uhd_bindings.rs")
        .file("src/hardware/uhd_wrapper.cpp")
        .include(&uhd_include)
        .include(&boost_include)
        .include("src/hardware")
        .flag_if_supported("-std=c++14")
        .flag_if_supported("-O3")
        .flag_if_supported("-march=native")
        .compile("uhd_wrapper");
    
    // Link UHD library
    let uhd_lib_dir = uhd_dir.join("lib");
    println!("cargo:rustc-link-search=native={}", uhd_lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=uhd");
    
    // Link Boost libraries if needed
    println!("cargo:rustc-link-lib=dylib=boost_system");
    println!("cargo:rustc-link-lib=dylib=boost_thread");
}

fn find_uhd_dir() -> PathBuf {
    // Try environment variable first
    if let Ok(uhd_dir) = env::var("UHD_DIR") {
        return PathBuf::from(uhd_dir);
    }
    
    // Try pkg-config
    if let Ok(output) = std::process::Command::new("pkg-config")
        .args(&["--variable=prefix", "uhd"])
        .output()
    {
        if output.status.success() {
            let prefix = String::from_utf8_lossy(&output.stdout);
            return PathBuf::from(prefix.trim());
        }
    }
    
    // Try common locations
    let common_paths = vec![
        "/usr/local",
        "/usr",
        "/opt/uhd",
        "C:\\Program Files\\UHD",
    ];
    
    for path in common_paths {
        let path = PathBuf::from(path);
        if path.join("include/uhd").exists() {
            return path;
        }
    }
    
    panic!("Could not find UHD installation. Please set UHD_DIR environment variable.");
}

fn find_boost_include() -> PathBuf {
    // Try environment variable
    if let Ok(boost_dir) = env::var("BOOST_ROOT") {
        return PathBuf::from(boost_dir).join("include");
    }
    
    // Try pkg-config
    if let Ok(output) = std::process::Command::new("pkg-config")
        .args(&["--cflags", "boost"])
        .output()
    {
        if output.status.success() {
            let flags = String::from_utf8_lossy(&output.stdout);
            for flag in flags.split_whitespace() {
                if flag.starts_with("-I") {
                    return PathBuf::from(&flag[2..]);
                }
            }
        }
    }
    
    // Try common locations
    let common_paths = vec![
        "/usr/include",
        "/usr/local/include",
        "/opt/boost/include",
        "C:\\boost\\include",
    ];
    
    for path in common_paths {
        let path = PathBuf::from(path);
        if path.join("boost/version.hpp").exists() {
            return path;
        }
    }
    
    // Default to system include
    PathBuf::from("/usr/include")
}