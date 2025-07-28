// src/modules/macmon/fetch.rs

use std::{
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use lazy_static::lazy_static;
use serde_json::Value;
use tokio::{
    spawn,
    sync::Mutex as TokioMutex,
    time::{interval},
};

lazy_static! {
    static ref CACHE: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
    static ref LAST_ACCESS: Arc<Mutex<Instant>> = Arc::new(Mutex::new(Instant::now()));
    static ref FETCHING: Arc<TokioMutex<bool>> = Arc::new(TokioMutex::new(false));
}

#[cfg(target_os = "macos")]
pub async fn fetch_macmon() -> Option<Value> {
    {
        let mut last_access = LAST_ACCESS.lock().unwrap();
        *last_access = Instant::now();
    }

    {
        let cache = CACHE.lock().unwrap();
        if cache.is_some() {
            return cache.clone();
        }
    }

    let mut fetching = FETCHING.lock().await;
    if !*fetching {
        *fetching = true;
        let cache_clone = CACHE.clone();
        let last_access_clone = LAST_ACCESS.clone();
        spawn(async move {
            let mut ticker = interval(Duration::from_millis(1000));

            loop {
                ticker.tick().await;

                {
                    let last = last_access_clone.lock().unwrap();
                    if last.elapsed() > Duration::from_secs(60) {
                        *cache_clone.lock().unwrap() = None;
                        break;
                    }
                }

                if let Ok(child) = Command::new("macmon")
                    .args(["pipe", "-s", "1", "-i", "500"])
                    .stdout(Stdio::piped())
                    .spawn()
                {
                    if let Ok(output) = child.wait_with_output() {
                        if let Ok(stdout) = String::from_utf8(output.stdout) {
                            if let Some(line) = stdout.lines().find(|l| l.trim_start().starts_with('{')) {
                                if let Ok(json) = serde_json::from_str::<Value>(line) {
                                    *cache_clone.lock().unwrap() = Some(json);
                                }
                            }
                        }
                    }
                }
            }

            *FETCHING.lock().await = false;
        });
    }

    None
}
