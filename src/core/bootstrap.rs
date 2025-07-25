// src/core/bootstrap.rs

use crate::common::log::{log, LogLevel};
use chrono::Local;
use sysinfo::{Disks, System};

pub fn init() {
    let cargo_version = env!("CARGO_PKG_VERSION");
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let sys = System::new_all();

    // OS and Kernel
    let mut os_info =
        System::long_os_version().unwrap_or_else(|| System::os_version().unwrap_or_default());
    // Correct "MacOS" to "macOS" as requested.
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

    // --- Format the final output strings ---
    let line1 = format!("{}{} {}", os_info, kernel_name, kernel_version);
    let line2 = format!(
        "{}({}) {} {} {} {}%",
        cpu_brand, core_count, arch, fs_type, mem_swap_str, used_ram_percent
    );


    // --- Log the information using standard println! ---

    println!();
    const MAGENTA: &str = "\x1b[35m";
    const RESET: &str = "\x1b[0m";

    // ANSI escape sequence for creating a terminal hyperlink
    const LINK: &str = "\x1b]8;;https://rfs.im\x07@rfshub\x1b]8;;\x07";

    println!("  {}{}{}{} (Preview)", MAGENTA, "▲ Twig ", cargo_version, RESET);
    println!("  - Timestamp: {}", timestamp);
    println!("  - Copyright:");
    println!("    ✓ 2025 © Canmi {}, rfs ecosystem", LINK);
    println!("    ✓ Released under the AGPL-3.0 License");
    println!("  - Environment:");
    println!("    ✓ {}", line1);
    println!("    ✓ {}", line2);
    println!("    ✓ {}", fid);
    println!();

    log(LogLevel::Info, "✓ Starting...");
}
