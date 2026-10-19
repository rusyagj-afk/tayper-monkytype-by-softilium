use clap::Parser;
use std::io::{self, Write};

#[derive(Parser, Debug)]
#[command(
    name = "softilium",
    version = "1.2.0",
    about = "High-precision humanized Monkeytype client using raw Chrome DevTools Protocol"
)]
pub struct CliArgs {
    /// Target typing speed in WPM (e.g. 100, 180, 200, 250)
    #[arg(short, long)]
    pub wpm: Option<f64>,

    /// Humanization mode: legit (with typos ~98%), smooth (wavy, 100%), flat (robotic)
    #[arg(short, long)]
    pub mode: Option<String>,

    /// Activation trigger: hotkey (F8 in browser), manual (first key), delay (8s), direct (3..2..1)
    #[arg(short, long)]
    pub trigger: Option<String>,

    /// Browser name or executable path (helium, chrome, edge, brave)
    #[arg(short, long)]
    pub browser: Option<String>,

    /// CDP host:port
    #[arg(long, default_value = "127.0.0.1:9222")]
    pub cdp: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TriggerMode {
    Hotkey,
    FirstKey,
    Delay,
    Direct,
}

pub fn prompt_input(label: &str, default: &str) -> String {
    print!("{label} [{default}]: ");
    let _ = io::stdout().flush();

    let mut buf = String::new();
    let _ = io::stdin().read_line(&mut buf);
    let trimmed = buf.trim();

    if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed.to_string()
    }
}
