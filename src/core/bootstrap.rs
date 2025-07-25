// src/core/bootstrap.rs

use crate::common::log;
use chrono::Local;
use sysinfo::{Disks, System};

pub fn init() {
    let cargo_version = env!("CARGO_PKG_VERSION");
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let sys = System::new_all();

    // --- Gather all required information first ---

    // OS and Kernel
    let mut os_info =
        System::long_os_version().unwrap_or_else(|| System::os_version().unwrap_or_default());
    // Correct "MacOS" to "macOS"
    if os_info.starts_with("MacOS") {
        os_info = os_info.replace("MacOS", "macOS");
    }
    let kernel_name = System::name().unwrap_or_default().to_lowercase();
    let kernel_version = System::kernel_version().unwrap_or_default();

    // CPU
    let cpus = sys.cpus();
    let cpu_brand = cpus.first().map(|cpu| cpu.brand().trim()).unwrap_or("");
    let core_count = cpus.len();
    let arch = System::cpu_arch().unwrap_or_else(|| "Unknown Arch".to_string());

    // Disk
    let disks = Disks::new_with_refreshed_list();
    let fs_type = disks
        .iter()
        .find(|d| d.mount_point() == std::path::Path::new("/"))
        .and_then(|d| d.file_system().to_str())
        .unwrap_or("Unknown FS")
        .to_string();

    // Memory and Swap
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    let total_ram_gb = (sys.total_memory() as f64 / GIB).round() as u64;
    let total_swap_gb = (sys.total_swap() as f64 / GIB).round() as u64;
    let used_ram_percent = (sys.used_memory() as f64 / sys.total_memory() as f64 * 100.0).round() as u64;
    let mem_swap_str = if total_swap_gb > 0 {
        format!("{}+{}GB", total_ram_gb, total_swap_gb)
    } else {
        format!("{}GB", total_ram_gb)
    };

    // Machine ID
    let fid = machine_uid::get().unwrap_or_else(|_| "Unavailable".to_string());

    // --- Format the final output strings with OS-specific logic ---
    let line1 = if cfg!(target_os = "linux") && os_info.to_lowercase().contains("debian") {
        format!("{} {}", os_info, kernel_version)
    } else if cfg!(target_os = "macos") {
        format!("{}{} {}", os_info, kernel_name, kernel_version)
    } else {
        format!("{} {} {}", os_info, kernel_name, kernel_version)
    };

    let line2 = format!(
        "{}({}) {} {} {} {}%",
        cpu_brand, core_count, arch, fs_type, mem_swap_str, used_ram_percent
    );

    log::println("");

    const MAGENTA: &str = "\x1b[35m";
    const RESET: &str = "\x1b[0m";
    const LINK: &str = "\x1b]8;;https://rfs.im\x07@rfshub\x1b]8;;\x07";

    log::println(&format!("  {}{}{}{} (Preview)", MAGENTA, "▲ Twig ", cargo_version, RESET));
    log::println(&format!("  - Timestamp: {}", timestamp));
    log::println("  - Copyright:");
    log::println(&format!("    ✓ 2025 © Canmi {}, rfs ecosystem", LINK));
    log::println("    ✓ Released under the AGPL-3.0 License");
    log::println("  - Environment:");
    log::println(&format!("    ✓ {}", line1));
    log::println(&format!("    ✓ {}", line2));
    log::println(&format!("    ✓ {}", fid));
    log::println("");
    log::log(log::LogLevel::Info, "✓ Starting...");
}
