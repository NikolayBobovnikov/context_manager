use std::process::Command;
use std::env;
use std::path::PathBuf;

fn main() {
    // Rerun this script if Cargo.toml changes
    println!("cargo:rerun-if-changed=Cargo.toml");

    // Get the build profile (debug or release)
    let profile = env::var("PROFILE").unwrap();

    // Only run this logic for release builds
    if profile == "release" {
        println!("Building release version, attempting to copy executable...");

        // Get the path to the compiled executable
        let target_dir = env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".to_string());
        let executable_name = "context_builder"; // Your binary name
        let source_path = PathBuf::from(&target_dir).join(profile).join(executable_name);

        // Get the user's home directory and construct the destination path
        if let Some(home_dir) = env::var_os("HOME") {
            let mut dest_dir = PathBuf::from(home_dir);
            dest_dir.push(".local");
            dest_dir.push("bin");

            let dest_path = dest_dir.join(executable_name);

            // Ensure the destination directory exists
            if let Err(e) = std::fs::create_dir_all(&dest_dir) {
                eprintln!("Warning: Could not create destination directory {:?}: {}", dest_dir, e);
                 // Continue even if directory creation fails, copy might still work if it exists
            }

            println!("Copying {:?} to {:?}...", source_path, dest_path);

            // Use the `install` command for robustness (creates dest dir if needed, handles permissions)
            // Alternatively, could use std::fs::copy
            let output = Command::new("cp")
                .arg(&source_path)
                .arg(&dest_path)
                .output();

            match output {
                Ok(output) => {
                    if output.status.success() {
                        println!("Successfully copied executable to {:?}", dest_path);
                    } else {
                        eprintln!("Error copying executable: {}", String::from_utf8_lossy(&output.stderr));
                    }
                }
                Err(e) => {
                    eprintln!("Error executing copy command: {}", e);
                }
            }
        } else {
            eprintln!("Warning: HOME environment variable not found, skipping executable copy.");
        }
    }
} 