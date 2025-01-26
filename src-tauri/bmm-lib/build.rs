// use std::process::Command;
use std::env;
use std::path::PathBuf;

fn main() {
    if std::env::var("SKIP_BUILD_SCRIPT").unwrap_or("0".to_string()) == "1" {
        return;
    }
    #[cfg(target_os = "macos")]
    {
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let lovely_mac_dir = PathBuf::from(&manifest_dir).join("../lovely-injector");
        let lovely_bin_dir = dirs::config_dir()
            .expect("Could not find config directory")
            .join("Balatro")
            .join("bins");
        //

        //
        // Get the correct dylib path
        let dylib_path = lovely_mac_dir.join("target/release/liblovely.dylib");

        if !dylib_path.exists() {
            panic!("Dylib not found at expected path: {}", dylib_path.display());
        }

        // move liblove.dylib to the correct location (config/Balatro/bins)
        std::fs::copy(&dylib_path, lovely_bin_dir.join("liblovely.dylib"))
            .expect("Failed to copy liblovely.dylib to bins directory");

        println!("cargo:rerun-if-changed={}", dylib_path.display());
    }
}
