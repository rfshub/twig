/* src/requirement.rs */

use crate::common::log;
use std::{env, process};
use std::process::Command;
use std::thread;
use std::time::Duration;

const MAC_COMMANDS: [&str; 6] = ["brew", "macmon", "netstat", "nettop", "fastfetch", "speedtest-cli"];
const LINUX_COMMANDS: [&str; 3] = ["linux-cpupower", "fastfetch", "speedtest"];

pub fn run_dependency_check() {
    let os = env::consts::OS;
    let commands_to_check = if os == "macos" {
        MAC_COMMANDS.as_ref()
    } else if os == "linux" {
        LINUX_COMMANDS.as_ref()
    } else {
        log::log(log::LogLevel::Error, &format!("✗ Unsupported OS: {}. Cannot check dependencies.", os));
        process::exit(1);
    };

    let mut missing_commands = Vec::new();
    let mut found_commands = Vec::new();

    for &cmd in commands_to_check {
        if which(cmd).is_some() {
            found_commands.push(cmd);
        } else {
            missing_commands.push(cmd);
        }
    }

    if !missing_commands.is_empty() {
        log::log(log::LogLevel::Error, "✗ Dependency check fail");
        log::log(log::LogLevel::Error, &format!("  ✗ {}", missing_commands.join(", ")));

        if !found_commands.is_empty() {
            log::log(log::LogLevel::Warn, &format!("  ✓ {}", found_commands.join(", ")));
        }

        let install_list = missing_commands.join(" ");
        if os == "macos" {
            if which("brew").is_none() {
                log::log(log::LogLevel::Error, "✗ Homebrew (brew) is not installed.");
                log::log(log::LogLevel::Error, "➜ Please install it first from github");
                log::log(log::LogLevel::Warn, "✓ https://github.com/Homebrew/brew");
            } else {
                log::log(log::LogLevel::Warn, "➜ Install missing pkg via homebrew");
                log::log(log::LogLevel::Warn, &format!("  ✓ brew install {}", install_list));
            }
        } else if os == "linux" {
            let distro = get_linux_distro();
            match distro.as_str() {
                "ubuntu" | "debian" => {
                    log::log(log::LogLevel::Warn, "➜ Install missing pkg via apt");
                    log::log(log::LogLevel::Warn, &format!("  ✓ apt update && apt install {}", install_list));
                }
                "arch" | "manjaro" => {
                    log::log(log::LogLevel::Warn, "➜ Install missing pkg via pacman or yay");
                    log::log(log::LogLevel::Warn, &format!("  ✓ pacman -Sy {}", install_list));
                }
                _ => {
                    log::log(log::LogLevel::Error, "➜ Please install the missing commands using your system's package manager.");
                    log::log(log::LogLevel::Warn, "✓ For example, on Fedora you might use `dnf`, on CentOS use `yum`, etc.");
                }
            }
        }

        if let Some(path) = crate::common::log::get_log_path() {
            thread::sleep(Duration::from_millis(1000));
            log::log(log::LogLevel::Error, &format!("✗ The crash report can be found at {}", path.display()));
        }

        thread::sleep(Duration::from_millis(500));
        process::exit(1);
    }
}

fn which(cmd: &str) -> Option<String> {
    Command::new("which")
        .arg(cmd)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !path.is_empty() {
                Some(path)
            } else {
                None
            }
        })
}

// Try to get a lowercase distro ID from /etc/os-release or fallback "unknown"
fn get_linux_distro() -> String {
    if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("ID=") {
                let id = rest.trim_matches('"').to_lowercase();
                return id;
            }
        }
    }
    "unknown".into()
}
