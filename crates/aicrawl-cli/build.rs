fn main() {
    println!("cargo:rerun-if-env-changed=RUSTC");
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    if let Ok(output) = std::process::Command::new(rustc).arg("--version").output() {
        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout);
            println!("cargo:rustc-env=AICRAWL_RUSTC_VERSION={}", version.trim());
        }
    }
}
