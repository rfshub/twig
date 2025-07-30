// src/common/setup.rs

use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    thread,
    time::Duration,
};

use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use rand::{rngs::OsRng, Rng, RngCore};
use sha2::{Digest, Sha256};

const SEED_SIZE: usize = 64;
const TOKEN_COUNT: usize = 6;
const PASSWD_PATH: &str = "/opt/rfs/twig/config/passwd";

pub fn init_token() {
    if Path::new(PASSWD_PATH).exists() {
        return;
    }

    let all_seeds = (0..TOKEN_COUNT)
        .flat_map(|_| generate_seed())
        .collect::<Vec<u8>>();

    save_seed_to_file(&all_seeds);
    thread::sleep(Duration::from_millis(2000));

    // === platform specific output ===
    #[cfg(target_os = "macos")]
    {
        let encoded = general_purpose::STANDARD.encode(&all_seeds);
        if let Err(e) = copy_to_clipboard(&encoded) {
            eprintln!("! Failed to copy to clipboard: {}", e);
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        print_seed_base64(&all_seeds);
    }

    thread::sleep(Duration::from_millis(500));
    print_seed_ascii(&all_seeds);
    thread::sleep(Duration::from_millis(500));

    #[cfg(target_os = "macos")]
    {
        println!("\n  Node key copied successfully to clipboard.");
        println!("  Please keep it properly. You will never see it again.\n");
    }

    #[cfg(not(target_os = "macos"))]
    {
        println!("\n  Node key generated successfully");
        println!("  Please keep it properly. You will never see it again.\n");
    }

    thread::sleep(Duration::from_millis(3000));
}

pub fn compute_token_windows() -> ([String; 6], [String; 6]) {
    let mut buf = [0u8; SEED_SIZE * TOKEN_COUNT];
    File::open(PASSWD_PATH)
        .expect("Token seed file not found")
        .read_exact(&mut buf)
        .expect("Failed to read token seeds");

    let now = Utc::now().timestamp() / 15;
    let times = [now - 1, now];
    let mut result = vec![];

    for &timestamp in &times {
        for i in 0..TOKEN_COUNT {
            let seed = &buf[i * SEED_SIZE..(i + 1) * SEED_SIZE];
            let mut hasher = Sha256::new();
            hasher.update(seed);
            hasher.update(timestamp.to_be_bytes());
            let hash = hasher.finalize();
            let number = u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]) % 1_000_000;
            result.push(format!("{:06}", number));
        }
    }

    let a: [String; 6] = result[..6].to_vec().try_into().unwrap();
    let b: [String; 6] = result[6..].to_vec().try_into().unwrap();
    (a, b)
}

/* --- Internal helpers --- */

fn generate_seed() -> [u8; SEED_SIZE] {
    let mut seed = [0u8; SEED_SIZE];
    OsRng.fill_bytes(&mut seed);
    seed
}

fn save_seed_to_file(data: &[u8]) {
    if let Some(dir) = Path::new(PASSWD_PATH).parent() {
        fs::create_dir_all(dir).expect("! Failed to create config directory");
    }

    let mut file = File::create(PASSWD_PATH).expect("! Failed to create token file");
    file.write_all(data).expect("! Failed to write token seeds");
}

#[cfg(not(target_os = "macos"))]
fn print_seed_base64(data: &[u8]) {
    let encoded = general_purpose::STANDARD.encode(data);
    println!("{}\n", encoded);
}

fn print_seed_ascii(data: &[u8]) {
    let mut rng = rand::thread_rng();
    for chunk in data.chunks(16) {
        let hex: String = chunk
            .iter()
            .map(|b| {
                let hex = format!("{:02X}", b);
                let c0 = if rng.gen_bool(0.3) { '0' } else { hex.chars().nth(0).unwrap() };
                let c1 = if rng.gen_bool(0.3) { '0' } else { hex.chars().nth(1).unwrap() };
                format!("{}{} ", c0, c1)
            })
            .collect();

        let ascii: String = chunk
            .iter()
            .map(|b| {
                if rng.gen_bool(0.5) {
                    '.'
                } else {
                    let c = *b as char;
                    if c.is_ascii_graphic() && c != ' ' {
                        c
                    } else {
                        '*'
                    }
                }
            })
            .collect();

        println!("  {:<48}|{: <16}|", hex, ascii);
    }
}

#[cfg(target_os = "macos")]
fn copy_to_clipboard(text: &str) -> Result<(), Box<dyn std::error::Error>> {
    use arboard::Clipboard;
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(text.to_owned())?;
    Ok(())
}
