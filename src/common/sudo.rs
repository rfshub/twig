use std::process::Command;

#[cfg(target_family = "unix")]
pub fn check_root() {
    let output = Command::new("whoami")
        .output()
        .expect("Failed to execute whoami");
    let username = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if username != "root" {
        panic!("Permission denied: Please run this program as root.");
    }
}
