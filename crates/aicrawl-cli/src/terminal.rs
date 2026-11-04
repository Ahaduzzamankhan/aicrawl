//! Minimal colour-aware output helpers. Diagnostics always go to stderr so
//! stdout stays machine-readable JSON.

use owo_colors::OwoColorize;

/// Whether ANSI colour is enabled for this invocation.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    color: bool,
}

impl Theme {
    /// Honours `--no-color` and the `NO_COLOR` convention.
    pub fn detect(no_color: bool) -> Self {
        let disabled = std::env::var_os("NO_COLOR").is_some();
        Self {
            color: !no_color && !disabled,
        }
    }

    /// Bold heading.
    pub fn bold(&self, text: &str) -> String {
        if self.color {
            text.bold().to_string()
        } else {
            text.to_string()
        }
    }

    /// Dim secondary text.
    pub fn dim(&self, text: &str) -> String {
        if self.color {
            text.dimmed().to_string()
        } else {
            text.to_string()
        }
    }

    /// Error text.
    pub fn error(&self, text: &str) -> String {
        if self.color {
            text.red().to_string()
        } else {
            text.to_string()
        }
    }

    /// Success marker.
    pub fn ok_mark(&self) -> String {
        if self.color {
            "✔".green().to_string()
        } else {
            "ok".to_string()
        }
    }

    /// Error marker.
    pub fn error_mark(&self) -> String {
        if self.color {
            "✖".red().to_string()
        } else {
            "error".to_string()
        }
    }

    /// Warning marker.
    pub fn warn_mark(&self) -> String {
        if self.color {
            "!".yellow().to_string()
        } else {
            "warn".to_string()
        }
    }
}

/// Prints a `label: value` line to stderr.
pub fn kv(theme: &Theme, label: &str, value: impl std::fmt::Display) {
    eprintln!("  {} {}", theme.dim(&format!("{label}:")), value);
}
