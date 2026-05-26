use anyhow::{anyhow, Result};
use chaser_oxide::{ChaserPage, ChaserProfile};
use fake::faker::address::en::StreetName;
use fake::faker::name::en::{FirstName, LastName};
use fake::Fake;
use std::fs;
use std::path::PathBuf;
use tauri::{Emitter, Manager};

pub fn gen_email() -> String {
    let len = rand::random_range(10..16usize);
    let name: String = (0..len)
        .map(|_| {
            let charset = b"abcdefghijklmnopqrstuvwxyz0123456789";
            charset[rand::random_range(0..charset.len())] as char
        })
        .collect();
    format!("{}@gmail.com", name)
}

pub fn gen_phone() -> String {
    let rest: String = (0..7)
        .map(|_| char::from_digit(rand::random_range(0..10), 10).unwrap())
        .collect();
    format!("202{}", rest)
}

pub fn get_phone_number() -> String {
    fs::read_to_string("data/phone.txt")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(gen_phone)
}

pub fn gen_password() -> String {
    const UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    const LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
    const DIGITS: &[u8] = b"0123456789";
    const SYMBOLS: &[u8] = b"!@#$%^";
    const ALL: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^";

    let mut p: Vec<u8> = vec![
        UPPER[rand::random_range(0..UPPER.len())],
        LOWER[rand::random_range(0..LOWER.len())],
        DIGITS[rand::random_range(0..DIGITS.len())],
        SYMBOLS[rand::random_range(0..SYMBOLS.len())],
    ];
    for _ in 4..12 {
        p.push(ALL[rand::random_range(0..ALL.len())]);
    }
    String::from_utf8(p).unwrap()
}

pub fn gen_visa_card() -> String {
    let mut digits: Vec<u32> = vec![4];
    for _ in 0..14 {
        digits.push(rand::random_range(0..10));
    }
    let sum: u32 = digits
        .iter()
        .enumerate()
        .map(|(i, &d)| {
            let mut v = d;
            if i % 2 == 0 {
                v *= 2;
            }
            if v > 9 {
                v -= 9;
            }
            v
        })
        .sum();
    digits.push((10 - (sum % 10)) % 10);
    digits
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join("")
}

pub fn gen_cvv() -> String {
    format!("{:03}", rand::random_range(100..999u32))
}

fn sanitize_profile_key(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' => ch,
            _ => '_',
        })
        .collect()
}

pub fn browser_window_class(namespace: &str, account_key: &str, slot: usize) -> String {
    let sanitized = sanitize_profile_key(account_key);
    let namespace = if namespace.is_empty() {
        "auth"
    } else {
        namespace
    };
    format!("chaser_{}_{}_{}", namespace, slot, sanitized)
}

pub fn browser_profile_dir(
    app: &tauri::AppHandle,
    namespace: &str,
    account_key: &str,
) -> Result<PathBuf> {
    let sanitized = sanitize_profile_key(account_key);
    let profile_name = if namespace.is_empty() {
        sanitized.clone()
    } else {
        format!("{}_{}", namespace, sanitized)
    };

    let legacy_base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/chrome_profiles");
    if legacy_base.exists() {
        let scoped_dir = legacy_base.join(&profile_name);
        let raw_dir = legacy_base.join(&sanitized);
        let target = if scoped_dir.exists() {
            scoped_dir
        } else if raw_dir.exists() {
            raw_dir
        } else {
            scoped_dir
        };

        fs::create_dir_all(&target)
            .map_err(|e| anyhow!("Không tạo được thư mục profile {}: {}", target.display(), e))?;

        // Fix crash restore bubble
        let prefs_path = target.join("Default").join("Preferences");
        if prefs_path.exists() {
            if let Ok(mut prefs) = fs::read_to_string(&prefs_path) {
                prefs = prefs.replace("\"exit_type\":\"Crashed\"", "\"exit_type\":\"Normal\"");
                prefs = prefs.replace(
                    "\"exit_type\":\"SessionCrashed\"",
                    "\"exit_type\":\"Normal\"",
                );
                let _ = fs::write(&prefs_path, prefs);
            }
        }

        return Ok(target);
    }

    let mut target = app
        .path()
        .app_data_dir()
        .map_err(|e| anyhow!("Không lấy được app data dir: {}", e))?;
    target.push("chrome_profiles");
    target.push(profile_name);
    fs::create_dir_all(&target)
        .map_err(|e| anyhow!("Không tạo được thư mục profile {}: {}", target.display(), e))?;

    // Fix crash restore bubble
    let prefs_path = target.join("Default").join("Preferences");
    if prefs_path.exists() {
        if let Ok(mut prefs) = fs::read_to_string(&prefs_path) {
            prefs = prefs.replace("\"exit_type\":\"Crashed\"", "\"exit_type\":\"Normal\"");
            prefs = prefs.replace(
                "\"exit_type\":\"SessionCrashed\"",
                "\"exit_type\":\"Normal\"",
            );
            let _ = fs::write(&prefs_path, prefs);
        }
    }

    Ok(target)
}

pub async fn apply_fingerprint_profile(
    chaser: &ChaserPage,
    app: &tauri::AppHandle,
    flow: &str,
    email: &str,
) -> Result<()> {
    let (locale, timezone) = ("en-US", "America/New_York");

    let profile_builder = if cfg!(target_os = "windows") {
        ChaserProfile::windows()
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        ChaserProfile::macos_arm()
    } else if cfg!(target_os = "macos") {
        ChaserProfile::macos_intel()
    } else {
        ChaserProfile::linux()
    };

    let profile = profile_builder
        .locale(locale)
        .timezone(timezone)
        .screen(1920, 1080)
        .build();

    chaser
        .apply_profile(&profile)
        .await
        .map_err(|e| anyhow!("Không áp dụng được fingerprint profile: {}", e))?;

    let _ = app.emit(
        "automation-log",
        format!(
            "🧬 [{}] [{}] Đã áp dụng fingerprint profile ({}, {}).",
            flow, email, locale, timezone
        ),
    );

    Ok(())
}

pub fn tiled_window_position(
    slot: usize,
    total_slots: usize,
    width: u32,
    height: u32,
) -> (u32, u32) {
    let total_slots = total_slots.max(1);
    let columns = ((total_slots as f64).sqrt().ceil() as usize).clamp(1, 3);

    let col = (slot % columns) as u32;
    let row = (slot / columns) as u32;
    let margin_x = 8;
    let margin_y = 32;
    let gap_x = 8;
    let gap_y = 8;

    let x = margin_x + col * (width + gap_x);
    let y = margin_y + row * (height + gap_y);
    (x, y)
}

#[cfg(target_os = "linux")]
pub async fn force_tile_window(window_class: &str, x: u32, y: u32, width: u32, height: u32) {
    // Wait for window to be created
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    for _ in 0..10 {
        let status = tokio::process::Command::new("xdotool")
            .args([
                "search",
                "--name",
                window_class,
                "windowsize",
                "%@",
                &width.to_string(),
                &height.to_string(),
                "windowmove",
                "%@",
                &x.to_string(),
                &y.to_string(),
            ])
            .status()
            .await;

        if matches!(status, Ok(exit) if exit.success()) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

#[cfg(not(target_os = "linux"))]
pub async fn force_tile_window(_window_class: &str, _x: u32, _y: u32, _width: u32, _height: u32) {}

const MATCHED_LOCATIONS: &[(&str, &str, &str)] = &[
    ("NY", "New York", "10001"),
    ("CA", "Los Angeles", "90001"),
    ("IL", "Chicago", "60601"),
    ("TX", "Houston", "77001"),
    ("PA", "Philadelphia", "19101"),
    ("AZ", "Phoenix", "85001"),
    ("TX", "San Antonio", "78201"),
    ("CA", "San Diego", "92101"),
    ("TX", "Dallas", "75201"),
    ("CA", "San Jose", "95101"),
    ("FL", "Miami", "33101"),
    ("GA", "Atlanta", "30301"),
    ("WA", "Seattle", "98101"),
    ("MA", "Boston", "02101"),
    ("CO", "Denver", "80201"),
    ("NV", "Las Vegas", "89101"),
    ("OR", "Portland", "97201"),
    ("MI", "Detroit", "48201"),
    ("DC", "Washington", "20001"),
    ("OH", "Columbus", "43201"),
];

pub async fn gen_random_billing_info() -> (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
) {
    let first: String = FirstName().fake();
    let last_name: String = LastName().fake();
    let street_name: String = StreetName().fake();
    let street = format!("{} {}", rand::random_range(100u32..9999), street_name);

    let idx = rand::random_range(0..MATCHED_LOCATIONS.len());
    let (county, city_name, postcode) = MATCHED_LOCATIONS[idx];

    let city = city_name.to_string();
    let state = county.to_string();
    let zip = postcode.to_string();

    let phone = gen_phone();
    let pass = gen_password();
    (first, last_name, street, city, state, zip, phone, pass)
}
