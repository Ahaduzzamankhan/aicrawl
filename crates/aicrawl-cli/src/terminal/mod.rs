//! Terminal presentation helpers.
//!
//! Colours are emitted only when stdout is a terminal, `NO_COLOR` is unset and
//! `--no-color` was not passed. Progress goes to stderr so JSON output can be
//! piped safely.

use std::io::IsTerminal;
use std::time::Duration;

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use owo_colors::OwoColorize;

/// Colour decision for the current process.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub enabled: bool,
}

impl Theme {
    /// Detects colour support from flags, `NO_COLOR` and TTY status.
    pub fn detect(no_color: bool) -> Self {
        let disabled =
            no_color || std::env::var_os("NO_COLOR").is_some() || !std::io::stdout().is_terminal();
        Self { enabled: !disabled }
    }

    /// Purple branding text.
    pub fn brand(&self, text: impl std::fmt::Display) -> String {
        self.paint(text, |t| t.as_str().bright_magenta().to_string())
    }

    /// Green success text.
    pub fn success(&self, text: impl std::fmt::Display) -> String {
        self.paint(text, |t| t.as_str().green().to_string())
    }

    /// Blue informational text.
    pub fn info(&self, text: impl std::fmt::Display) -> String {
        self.paint(text, |t| t.as_str().blue().to_string())
    }

    /// Yellow warning/progress text.
    pub fn warn(&self, text: impl std::fmt::Display) -> String {
        self.paint(text, |t| t.as_str().yellow().to_string())
    }

    /// Red error text.
    pub fn error(&self, text: impl std::fmt::Display) -> String {
        self.paint(text, |t| t.as_str().red().to_string())
    }

    /// Dim secondary text.
    pub fn dim(&self, text: impl std::fmt::Display) -> String {
        self.paint(text, |t| t.as_str().dimmed().to_string())
    }

    /// Bold text.
    pub fn bold(&self, text: impl std::fmt::Display) -> String {
        self.paint(text, |t| t.as_str().bold().to_string())
    }

    /// Cyan accent text.
    pub fn accent(&self, text: impl std::fmt::Display) -> String {
        self.paint(text, |t| t.as_str().cyan().to_string())
    }

    fn paint(&self, text: impl std::fmt::Display, style: impl Fn(String) -> String) -> String {
        let text = text.to_string();
        if self.enabled {
            style(text)
        } else {
            text
        }
    }

    /// `✓`
    pub fn ok_mark(&self) -> String {
        self.success("✓")
    }

    /// `✗`
    pub fn err_mark(&self) -> String {
        self.error("✗")
    }

    /// `!`
    pub fn warn_mark(&self) -> String {
        self.warn("!")
    }

    /// `›`
    pub fn arrow(&self) -> String {
        self.dim("›")
    }
}

/// Prints the AICrawl banner.
pub fn print_banner(theme: &Theme) {
    println!(
        "{} {}",
        theme.brand("✦ AICrawl"),
        theme.dim(format!("v{}", aicrawl_core::VERSION))
    );
    println!("{}", theme.dim("AI-powered web extraction engine"));
    println!();
}

/// Prints a section title.
pub fn section(theme: &Theme, title: &str) {
    println!();
    println!("{}", theme.bold(title));
}

/// Prints a key/value line with aligned value.
pub fn kv(theme: &Theme, key: &str, value: impl std::fmt::Display) {
    println!("  {} {}", theme.dim(format!("{key:<14}")), value);
}

/// Prints a small table. Values are plain so column widths remain accurate.
pub fn table(theme: &Theme, headers: &[&str], rows: &[Vec<String>]) {
    if rows.is_empty() {
        println!("{}", theme.dim("  (none)"));
        return;
    }
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            if index < widths.len() {
                widths[index] = widths[index].max(cell.chars().count());
            }
        }
    }
    let header_line: Vec<String> = headers
        .iter()
        .enumerate()
        .map(|(index, header)| format!("{header:<width$}", width = widths[index]))
        .collect();
    println!("  {}", theme.dim(header_line.join("  ")));
    for row in rows {
        let cells: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(index, cell)| {
                let width = widths.get(index).copied().unwrap_or(cell.len());
                format!("{cell:<width$}")
            })
            .collect();
        println!("  {}", cells.join("  "));
    }
}

/// Spinner that stays silent on non-TTY output.
pub struct Spinner {
    bar: ProgressBar,
}

impl Spinner {
    /// Starts a spinner with `message` (stderr).
    pub fn start(theme: &Theme, message: impl Into<String>) -> Self {
        let bar = if theme.enabled && std::io::stderr().is_terminal() {
            let bar = ProgressBar::new_spinner();
            bar.set_draw_target(ProgressDrawTarget::stderr());
            bar.set_style(
                ProgressStyle::with_template("{spinner:.magenta} {msg}")
                    .expect("spinner template")
                    .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
            );
            bar.enable_steady_tick(Duration::from_millis(80));
            bar.set_message(message.into());
            bar
        } else {
            ProgressBar::hidden()
        };
        Self { bar }
    }

    /// Updates the spinner message.
    pub fn set(&self, message: impl Into<String>) {
        self.bar.set_message(message.into());
    }

    /// Handle that can be moved into a progress callback.
    pub fn bar(&self) -> ProgressBar {
        self.bar.clone()
    }

    /// Clears the spinner (call before printing the final result).
    pub fn clear(&self) {
        self.bar.finish_and_clear();
    }

    /// Clears the spinner and prints a success line to stderr.
    pub fn finish(&self, theme: &Theme, message: impl std::fmt::Display) {
        self.bar.finish_and_clear();
        eprintln!("{} {}", theme.ok_mark(), message);
    }

    /// Clears the spinner without printing.
    pub fn fail(&self) {
        self.bar.finish_and_clear();
    }
}
