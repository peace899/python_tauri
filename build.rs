use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Python configuration
    let python_executable = "python"; // or specify full path if needed
    let _python_include_dir = get_python_include_dir(python_executable);
    let python_library = get_python_library(python_executable);
    let python_version = get_python_version(python_executable);
   
    println!("cargo:rustc-link-search=native={}", python_library.display());
    println!("cargo:rustc-link-lib=python{}", python_version); // Replace with your Python version (e.g., 39 for 3.9)

    // Tauri configuration
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rustc-cfg=feature=\"events-plugin\"");
    // Set up Tauri bundler
    tauri_build::build();
}

fn get_python_include_dir(python_executable: &str) -> PathBuf {
    let output = Command::new(python_executable)
        .args(["-c", "import sysconfig; print(sysconfig.get_path('include'))"])
        .output()
        .expect("Failed to execute Python to get include path");
    
    PathBuf::from(String::from_utf8(output.stdout).unwrap().trim())
}

fn get_python_library(python_executable: &str) -> PathBuf {
    let output = Command::new(python_executable)
        .args(["-c", r#"
import sysconfig
import os
import sys

libs = []
if sys.platform == 'win32':
    libs.append(os.path.join(sysconfig.get_config_var('installed_base'), 'libs'))
    libs.append(os.path.join(sys.base_prefix, 'libs'))
    
for lib in libs:
    if os.path.exists(lib):
        print(lib)
        break
"#])
        .output()
        .expect("Failed to execute Python to get library path");
    
    PathBuf::from(String::from_utf8(output.stdout).unwrap().trim())
}

fn get_python_version(python_executable: &str) -> String {
    let output = Command::new(python_executable)
        .args(["-c", "import sys; print(sys.winver.replace('.',''))"])
        .output()
        .expect("Failed to execute Python to get version");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}