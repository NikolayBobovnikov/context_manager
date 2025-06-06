use std::process::Command;
use std::env;
use std::path::PathBuf;

fn main() {
    // Remove the `rerun-if-changed` directive to ensure the script runs unconditionally on every build.
    // println!("cargo:rerun-if-changed=Cargo.toml");

    // Get the build profile (debug or release) - no longer conditionally checked
    let profile = env::var("PROFILE").unwrap();

    // Always attempt to copy the executable
    println!("Attempting to copy executable for {} build...", profile);

    // Get the path to the compiled executable
    let target_dir = env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".to_string());
    let executable_name = "context_builder"; // Your binary name
    let source_path = PathBuf::from(&target_dir).join(profile).join(executable_name);

    // Get the user's home directory and construct the destination path
    if let Some(home_dir) = env::var_os("HOME") {
        let mut dest_dir = PathBuf::from(home_dir);
        dest_dir.push(".local");
        dest_dir.push("bin");

        // Use the `install` command for robustness (creates dest dir if needed, handles permissions)
        // It automatically handles directory creation if -D is used, or if dest is a directory.
        // We'll specify the destination directory and let `install` handle the filename.
        let output = Command::new("install")
            .arg(&source_path)
            .arg(&dest_dir)
            .output();

        match output {
            Ok(output) => {
                if output.status.success() {
                    println!("Successfully copied executable to {:?}", dest_dir.join(executable_name));
                } else {
                    eprintln!("Error copying executable: {}", String::from_utf8_lossy(&output.stderr));
                }
            }
            Err(e) => {
                eprintln!("Error executing install command: {}", e);
            }
        }
    } else {
        eprintln!("Warning: HOME environment variable not found, skipping executable copy.");
    }
} 