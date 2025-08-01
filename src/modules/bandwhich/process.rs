/* src/modules/bandwhich/process.rs */
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
pub struct ProcessInfo {
    pub name: String,
    pub upload_bps: u64,
    pub download_bps: u64,
}

type ProcessCache = Arc<Mutex<Vec<ProcessInfo>>>;

// Global cache for process network usage.
static PROCESS_CACHE: Lazy<ProcessCache> = Lazy::new(|| Arc::new(Mutex::new(Vec::new())));
// Tracks the last time the API was accessed to manage the background thread's lifecycle.
static LAST_ACCESS: Lazy<Arc<Mutex<Instant>>> = Lazy::new(|| Arc::new(Mutex::new(Instant::now())));
// A flag to ensure only one instance of the listener thread is running.
static IS_RUNNING: Lazy<Arc<Mutex<bool>>> = Lazy::new(|| Arc::new(Mutex::new(false)));

/// Parses a single line of `bandwhich --raw` output.
fn parse_line(line: &str) -> Option<ProcessInfo> {
    // Use a lazy_static regex for performance.
    static RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r#"process: <\d+> "(?P<name>.*?)" up/down Bps: (?P<up>\d+)/(?P<down>\d+) connections: \d+"#).unwrap()
    });
    RE.captures(line).and_then(|cap| {
        let name = cap.name("name").map_or("", |m| m.as_str()).to_string();
        let upload_bps = cap.name("up").and_then(|m| m.as_str().parse::<u64>().ok()).unwrap_or(0);
        let download_bps = cap.name("down").and_then(|m| m.as_str().parse::<u64>().ok()).unwrap_or(0);
        Some(ProcessInfo {
            name,
            upload_bps,
            download_bps,
        })
    })
}

/// Spawns a background thread to run `bandwhich` and parse its output.
/// This thread now restarts the bandwhich process every 20 seconds.
fn start_bandwhich_listener() {
    let cache = PROCESS_CACHE.clone();
    let last_access = LAST_ACCESS.clone();
    let running_flag = IS_RUNNING.clone();

    thread::spawn(move || {
        // This outer loop manages the 60-second inactivity timeout.
        loop {
            if last_access.lock().unwrap().elapsed() > Duration::from_secs(60) {
                println!("[twig] bandwhich listener timing out due to inactivity.");
                *cache.lock().unwrap() = Vec::new(); // Clear cache on exit.
                *running_flag.lock().unwrap() = false;
                break; // Exit the management loop.
            }

            let mut child = match Command::new("bandwhich")
                .arg("--raw")
                .stdout(Stdio::piped())
                .stderr(Stdio::null()) // Ignore stderr to avoid polluting logs.
                .spawn()
            {
                Ok(child) => child,
                Err(e) => {
                    eprintln!("[twig] Failed to start bandwhich: {}. Is it installed and in your PATH?", e);
                    *running_flag.lock().unwrap() = false;
                    return;
                }
            };

            if let Some(stdout) = child.stdout.take() {
                let reader = BufReader::new(stdout);
                let mut current_processes = Vec::new();
                let restart_timer = Instant::now();

                // This inner loop reads from a single `bandwhich` instance.
                for line in reader.lines() {
                    // Restart the process every 20 seconds to prevent hangs.
                    if restart_timer.elapsed() > Duration::from_secs(20) {
                        break; // Break from the read loop to restart the process.
                    }

                    match line {
                        Ok(line_str) => {
                            if line_str.starts_with("Refreshing:") {
                                // A new batch of data is starting. Update the global cache with the previous batch.
                                let mut cache_lock = cache.lock().unwrap();
                                *cache_lock = current_processes.clone();
                                // Clear the temporary list for the new batch.
                                current_processes.clear();
                            } else if let Some(proc_info) = parse_line(&line_str) {
                                current_processes.push(proc_info);
                            }
                        }
                        Err(_) => {
                            // This error can happen if the process is killed, which is an expected part of the timeout logic.
                            break; // Exit the read loop.
                        }
                    }
                }
            }
            // Kill the current child process before restarting in the next outer loop iteration.
            let _ = child.kill();
            let _ = child.wait(); // Clean up the zombie process.
        }
    });
}

/// Gets the list of processes and their network usage from `bandwhich`.
pub fn get_bandwhich_process() -> Vec<ProcessInfo> {
    // Update the last access time on every call.
    *LAST_ACCESS.lock().unwrap() = Instant::now();

    // Check if the listener thread is already running.
    let mut is_running = IS_RUNNING.lock().unwrap();
    if !*is_running {
        *is_running = true;
        // Drop the lock before spawning the thread to avoid deadlocks.
        drop(is_running);
        start_bandwhich_listener();
        // Give the listener a moment to start and perform the first data capture.
        // `bandwhich` can take a second or two to initialize its packet capture.
        thread::sleep(Duration::from_millis(2000));
    }

    // Return a clone of the current data from the cache.
    PROCESS_CACHE.lock().unwrap().clone()
}
