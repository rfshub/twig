// src/modules/iostat/pipeline.rs

use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    process::Stdio, // 只保留 Stdio
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    process::Command, // 使用 Tokio 的 Command
    spawn,
    sync::Mutex as TokioMutex,
    time::{interval},
};

/// 代表单个磁盘I/O统计信息的数据结构。
/// The data structure for a single disk's I/O statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskStat {
    pub kb_per_transfer: f64,
    pub transfers_per_second: f64,
    pub mb_per_second: f64,
}

/// 缓存类型的别名，用于存储从磁盘名称到其统计数据的映射。
/// Type alias for the cache, a map from disk name (e.g., "disk0") to its stats.
type IostatCache = Option<HashMap<String, DiskStat>>;

lazy_static! {
    // 全局静态变量，用于缓存、跟踪访问时间和控制后台任务。
    // Global statics for caching, tracking access time, and controlling the fetch task.
    static ref CACHE: Arc<Mutex<IostatCache>> = Arc::new(Mutex::new(None));
    static ref LAST_ACCESS: Arc<Mutex<Instant>> = Arc::new(Mutex::new(Instant::now()));
    static ref FETCHING: Arc<TokioMutex<bool>> = Arc::new(TokioMutex::new(false));
}

/// 解析 `iostat` 命令的输出。
///
/// 此函数专门解析 `iostat -d -c 2 -w 1` 的输出，该命令会打印两份报告后退出。
/// 我们只关心第二份（也就是最后一份）报告，因为它反映了最近时间间隔内的数据。
///
/// Parses the output of the `iostat` command.
/// It is designed for `iostat -d -c 2 -w 1`, which prints two reports and exits.
/// We parse the second (last) report, which reflects the most recent interval.
fn parse_iostat_output(output: &str) -> Option<HashMap<String, DiskStat>> {
    let lines: Vec<&str> = output.lines().filter(|l| !l.trim().is_empty()).collect();

    // 至少需要3行：磁盘名、表头、数据。`iostat -d -c 2` 会产生4行。
    // We need at least 3 lines: disks, headers, and one line of data.
    if lines.len() < 3 {
        return None;
    }

    let disk_names: Vec<String> = lines[0]
        .split_whitespace()
        .map(String::from)
        .collect();
    
    // 最后一行包含了我们需要的最新数据。
    // The last line contains the data for the most recent interval.
    let last_line = lines.last()?;
    let values: Vec<f64> = last_line
        .split_whitespace()
        .filter_map(|s| s.parse::<f64>().ok())
        .collect();

    // 每个磁盘有3个指标，数据量必须匹配。
    // Each disk has 3 metrics, so the number of values must match.
    if values.is_empty() || values.len() != disk_names.len() * 3 {
        return None;
    }

    let mut stats_map = HashMap::new();
    for (i, disk_name) in disk_names.iter().enumerate() {
        let start_index = i * 3;
        let stat = DiskStat {
            kb_per_transfer: *values.get(start_index).unwrap_or(&0.0),
            transfers_per_second: *values.get(start_index + 1).unwrap_or(&0.0),
            mb_per_second: *values.get(start_index + 2).unwrap_or(&0.0),
        };
        stats_map.insert(disk_name.clone(), stat);
    }

    Some(stats_map)
}

/// 获取 iostat 数据，使用懒加载、自动刷新和过期的缓存策略。
///
/// 第一次调用会启动一个后台任务来周期性获取数据。
/// 后续调用将直接返回缓存的数据。
/// 如果数据在60秒内未被访问，缓存将被清除，后台任务也会停止。
///
/// Fetches iostat data, using a lazy-loaded, auto-refreshing, and expiring cache.
/// The first call spawns a background task. Subsequent calls return cached data.
/// If not accessed for 60 seconds, the cache is cleared and the task stops.
pub async fn fetch_iostat() -> IostatCache {
    {
        // 每次调用都更新最后访问时间。
        // Update last access time on every call.
        let mut last_access = LAST_ACCESS.lock().unwrap();
        *last_access = Instant::now();
    }

    {
        // 优先检查缓存。
        // Check cache first for a quick return.
        let cache = CACHE.lock().unwrap();
        if cache.is_some() {
            return cache.clone();
        }
    }

    // 如果缓存为空，尝试启动后台获取任务。
    // If cache is empty, try to start the fetching process.
    let mut fetching = FETCHING.lock().await;
    if !*fetching {
        *fetching = true;
        
        let cache_clone = CACHE.clone();
        let last_access_clone = LAST_ACCESS.clone();
        
        spawn(async move {
            // 每2秒获取一次数据。
            // Fetch data every 2 seconds.
            let mut ticker = interval(Duration::from_secs(2));

            loop {
                ticker.tick().await;

                // 检查缓存是否还需要。
                // Check if the cache is still needed.
                {
                    let last = last_access_clone.lock().unwrap();
                    if last.elapsed() > Duration::from_secs(60) {
                        *cache_clone.lock().unwrap() = None;
                        break; // 停止任务
                    }
                }

                // 执行 iostat 命令。
                // `-c 2` 使其运行两次后退出，避免进程悬挂。
                // Execute the iostat command.
                if let Ok(output) = Command::new("iostat") // 现在是 tokio::process::Command
                    .args(["-d", "-c", "2", "-w", "1"])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .output()
                    .await // 现在可以正确 .await
                {
                    if let Ok(stdout) = String::from_utf8(output.stdout) {
                        if let Some(parsed_data) = parse_iostat_output(&stdout) {
                            *cache_clone.lock().unwrap() = Some(parsed_data);
                        }
                    }
                }
            }

            // 任务结束后，释放 fetching 锁。
            // Release the fetching lock once the loop is broken.
            *FETCHING.lock().await = false;
        });
    }

    // 首次调用或缓存清空后，立即返回 None，数据将由后台任务填充。
    // Return None initially; the cache will be populated by the background task.
    None
}
