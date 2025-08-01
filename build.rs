// build.rs
use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let target = env::var("TARGET").unwrap();

    if target != "aarch64-unknown-linux-musl" {
        return;
    }

    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let openssl_install_dir = out_dir.join("openssl-install");

    if openssl_install_dir.join("lib/libssl.a").exists() {
        println!("cargo:rustc-env=OPENSSL_DIR={}", openssl_install_dir.display());
        println!("cargo:rustc-env=OPENSSL_STATIC=true");
        return;
    }

    let openssl_version = "openssl-1.1.1w";
    let url = format!("https://www.openssl.org/source/{}.tar.gz", openssl_version);
    let openssl_tarball = out_dir.join(format!("{}.tar.gz", openssl_version));
    if !openssl_tarball.exists() {
        assert!(Command::new("curl")
            .arg("-L")
            .arg(url)
            .arg("-o")
            .arg(&openssl_tarball)
            .status()
            .expect("Failed to download OpenSSL")
            .success());
    }

    let openssl_src_dir = out_dir.join(openssl_version);
    if !openssl_src_dir.exists() {
        assert!(Command::new("tar")
            .arg("xzf")
            .arg(&openssl_tarball)
            .arg("-C")
            .arg(&out_dir)
            .status()
            .expect("Failed to extract OpenSSL")
            .success());
    }

    // OpenSSL
    let mut configure = Command::new("./Configure");
    configure
        .current_dir(&openssl_src_dir)
        .arg("linux-aarch64") // OpenSSL 的 Configure 脚本认识的目标
        .arg(format!("--prefix={}", openssl_install_dir.display()))
        .arg("no-shared")
        .arg("no-async");

    // Corss-builder CFLAGS
    configure.env("CFLAGS", "-fPIC -D_FORTIFY_SOURCE=0 -std=gnu11");

    assert!(configure
        .status()
        .expect("Failed to configure OpenSSL")
        .success());

    assert!(Command::new("make")
        .arg("-j")
        .arg(num_cpus::get().to_string())
        .current_dir(&openssl_src_dir)
        .env("CC", "aarch64-linux-gnu-gcc")
        .status()
        .expect("Failed to build OpenSSL")
        .success());

    assert!(Command::new("make")
        .arg("install_sw") // install_sw
        .current_dir(&openssl_src_dir)
        .status()
        .expect("Failed to install OpenSSL")
        .success());

    // openssl-sys crate
    println!("cargo:rustc-env=OPENSSL_DIR={}", openssl_install_dir.display());
    println!("cargo:rustc-env=OPENSSL_STATIC=true");
}
