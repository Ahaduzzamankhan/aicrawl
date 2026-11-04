//! Embeds the release version from the repository-root `version.json`.
//!
//! Release builds may set `AICRAWL_RELEASE_VERSION` (the release workflow does
//! this) so the tag and binary version can never disagree. Local builds fall
//! back to `version.json`, and finally to the crate version if the file is
//! unavailable (for example when the crate is vendored on its own).

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=../../version.json");
    println!("cargo:rerun-if-env-changed=AICRAWL_RELEASE_VERSION");

    let version = env::var("AICRAWL_RELEASE_VERSION")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .or_else(read_version_json)
        .unwrap_or_else(|| {
            env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION is set by Cargo")
        });

    println!("cargo:rustc-env=AICRAWL_VERSION={version}");
}

fn read_version_json() -> Option<String> {
    let manifest_dir = env::var_os("CARGO_MANIFEST_DIR")?;
    let path = PathBuf::from(manifest_dir).join("../../version.json");
    let contents = fs::read_to_string(path).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&contents).ok()?;
    let version = parsed.get("version")?.as_str()?.trim();
    (!version.is_empty()).then(|| version.to_owned())
}
