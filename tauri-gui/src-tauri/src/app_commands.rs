use crate::app_state::AppState;
use crate::file_store::{read_file_safe, read_lines_safe};
use crate::paths::*;
use rusqlite::Connection;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use tauri::{Manager, State};

#[derive(serde::Serialize)]
pub struct InitialData {
    accounts_list: String,
    access_tokens: String,
    stripe_links: String,
    paypal_links: String,
    profile_run_ips: String,
    phone: String,
    success_emails: Vec<String>,
    trial_registered: String,
    plus_verified_real: Vec<String>,
    total_links_count: usize,
    success_count: usize,
}

#[derive(serde::Serialize)]
pub struct Stats {
    total: usize,
    success: usize,
    success_emails: Vec<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupChromeProfilesResult {
    path: String,
    removed_files: usize,
    removed_dirs: usize,
    freed_bytes: u64,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAccountBrowserResult {
    opened: bool,
    profile_path: String,
    has_web_session: bool,
    has_access_token: bool,
    message: String,
}

#[tauri::command]
pub fn get_initial_data() -> Result<InitialData, String> {
    let accounts_list = read_file_safe(ACCOUNTS_LIST_FILE);
    let access_tokens = read_file_safe(ACCESS_TOKENS_FILE);
    let stripe_links = read_file_safe(STRIPE_LINKS_FILE);
    let paypal_links = read_file_safe(PAYPAL_LINKS_FILE);
    let profile_run_ips = read_file_safe(PROFILE_RUN_IPS_FILE);
    let phone = read_file_safe(PHONE_FILE);
    let success_emails = read_lines_safe(SUCCESS_FILE);
    let trial_registered = read_file_safe(TRIAL_REGISTERED_FILE);
    let plus_verified_real = read_lines_safe(PLUS_VERIFIED_REAL_FILE);

    let total_links_count = paypal_links
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count();
    let success_count = success_emails.len();

    Ok(InitialData {
        accounts_list,
        access_tokens,
        stripe_links,
        paypal_links,
        profile_run_ips,
        phone,
        success_emails,
        trial_registered,
        plus_verified_real,
        total_links_count,
        success_count,
    })
}

#[tauri::command]
pub fn get_stats() -> Result<Stats, String> {
    let links = read_file_safe(PAYPAL_LINKS_FILE);
    let total = links.lines().filter(|l| !l.trim().is_empty()).count();
    let success_emails = read_lines_safe(SUCCESS_FILE);
    let success = success_emails.len();
    Ok(Stats {
        total,
        success,
        success_emails,
    })
}

#[tauri::command]
pub fn save_file_content(file_type: String, content: String) -> Result<(), String> {
    let path = match file_type.as_str() {
        "accounts_list" => ACCOUNTS_LIST_FILE,
        "access_tokens" => ACCESS_TOKENS_FILE,
        "stripe_links" => STRIPE_LINKS_FILE,
        "trial_registered" => TRIAL_REGISTERED_FILE,
        "success_emails" => SUCCESS_FILE,
        "paypal_links" => PAYPAL_LINKS_FILE,
        _ => PAYPAL_LINKS_FILE,
    };

    if let Some(parent) = Path::new(path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(path, content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_settings(phone: String) -> Result<(), String> {
    fs::write(PHONE_FILE, phone).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_folder(path: String) -> Result<(), String> {
    let target = PathBuf::from(path);
    if !target.exists() {
        return Err(format!("Đường dẫn không tồn tại: {}", target.display()));
    }
    let folder = if target.is_dir() {
        target
    } else {
        target
            .parent()
            .ok_or_else(|| format!("Không tìm được thư mục cha của {}", target.display()))?
            .to_path_buf()
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut cmd = StdCommand::new("explorer");
        cmd.arg(&folder);
        cmd
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = StdCommand::new("open");
        cmd.arg(&folder);
        cmd
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut cmd = StdCommand::new("xdg-open");
        cmd.arg(&folder);
        cmd
    };

    command
        .spawn()
        .map_err(|e| format!("Không mở được thư mục {}: {}", folder.display(), e))?;

    Ok(())
}

#[tauri::command]
pub fn open_account_browser(
    app: tauri::AppHandle,
    email: String,
) -> Result<OpenAccountBrowserResult, String> {
    let trimmed_email = email.trim();
    if trimmed_email.is_empty() {
        return Err("Email không hợp lệ.".to_string());
    }

    let sanitize = |value: &str| -> String {
        value
            .chars()
            .map(|ch| match ch {
                'a'..='z' | 'A'..='Z' | '0'..='9' => ch,
                _ => '_',
            })
            .collect()
    };
    let sanitized = sanitize(trimmed_email);
    let legacy_base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(CHROME_PROFILES_DIR);
    let mut app_data_base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Không lấy được app data dir: {}", e))?;
    app_data_base.push("chrome_profiles");

    let candidate_names = [
        sanitized.clone(),
        format!("auth_{}", sanitized),
        format!("paypal_{}", sanitized),
    ];

    let mut existing_candidates: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
    for base in vec![legacy_base, app_data_base] {
        for name in &candidate_names {
            let path = base.join(name);
            if path.exists() && path.is_dir() {
                let modified = fs::metadata(&path)
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                existing_candidates.push((path, modified));
            }
        }
    }
    existing_candidates.sort_by_key(|(_, modified)| *modified);
    let profile_dir = existing_candidates
        .last()
        .map(|(path, _)| path.clone())
        .ok_or_else(|| {
            format!(
                "Chưa có profile local cho {}. Hãy chạy account này ít nhất 1 lần rồi mở lại.",
                trimmed_email
            )
        })?;

    let cookies_path = profile_dir.join("Default").join("Cookies");
    let has_web_session = if cookies_path.exists() {
        match Connection::open(&cookies_path) {
            Ok(conn) => {
                let count: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM cookies
                         WHERE (host_key = 'chatgpt.com' OR host_key = '.chatgpt.com')
                           AND (name LIKE '%session-token%' OR name = '__Secure-next-auth.session-token' OR name = 'next-auth.session-token')",
                        [],
                        |row| row.get(0),
                    )
                    .unwrap_or(0);
                count > 0
            }
            Err(_) => false,
        }
    } else {
        false
    };

    let has_access_token = read_file_safe(ACCESS_TOKENS_FILE).lines().any(|line| {
        line.split_once('|')
            .map(|(stored_email, token)| {
                stored_email.trim().eq_ignore_ascii_case(trimmed_email) && !token.trim().is_empty()
            })
            .unwrap_or(false)
    });

    if !has_web_session {
        if has_access_token {
            return Err(format!(
                "Acc {} có Access Token API nhưng chưa có web session cookie trong profile. Hãy chạy luồng login web cho acc này trước.",
                trimmed_email
            ));
        }
        return Err(format!(
            "Acc {} chưa có web session cookie trong profile nên chưa mở trực tiếp được.",
            trimmed_email
        ));
    }

    let profile_arg = format!("--user-data-dir={}", profile_dir.display());

    #[cfg(target_os = "windows")]
    let browser_candidates: &[&str] = &["chrome.exe", "msedge.exe"];
    #[cfg(target_os = "macos")]
    let browser_candidates: &[&str] = &[
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    ];
    #[cfg(all(unix, not(target_os = "macos")))]
    let browser_candidates: &[&str] = &[
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
        "microsoft-edge",
    ];

    let mut launch_errors = Vec::new();
    for browser in browser_candidates {
        match StdCommand::new(browser)
            .arg("--new-window")
            .arg("--profile-directory=Default")
            .arg(&profile_arg)
            .arg("https://chatgpt.com")
            .spawn()
        {
            Ok(_) => {
                return Ok(OpenAccountBrowserResult {
                    opened: true,
                    profile_path: profile_dir.display().to_string(),
                    has_web_session,
                    has_access_token,
                    message: format!("Đã mở browser cho {}", trimmed_email),
                })
            }
            Err(e) => launch_errors.push(format!("{}: {}", browser, e)),
        }
    }

    Err(format!(
        "Không mở được browser cho {}. Đã thử: {}",
        trimmed_email,
        launch_errors.join(" | ")
    ))
}

fn count_path_contents(path: &Path) -> Result<(usize, usize, u64), String> {
    if !path.exists() {
        return Ok((0, 0, 0));
    }

    let mut files = 0usize;
    let mut dirs = 0usize;
    let mut bytes = 0u64;

    for entry in
        fs::read_dir(path).map_err(|e| format!("Không đọc được {}: {}", path.display(), e))?
    {
        let entry =
            entry.map_err(|e| format!("Không đọc được entry trong {}: {}", path.display(), e))?;
        let entry_path = entry.path();
        let metadata = fs::symlink_metadata(&entry_path)
            .map_err(|e| format!("Không đọc được metadata {}: {}", entry_path.display(), e))?;

        if metadata.is_dir() {
            dirs += 1;
            let (child_files, child_dirs, child_bytes) = count_path_contents(&entry_path)?;
            files += child_files;
            dirs += child_dirs;
            bytes += child_bytes;
        } else {
            files += 1;
            bytes += metadata.len();
        }
    }

    Ok((files, dirs, bytes))
}

#[tauri::command]
pub fn cleanup_chrome_profiles(
    state: State<'_, AppState>,
) -> Result<CleanupChromeProfilesResult, String> {
    if state.running_task.lock().unwrap().is_some() {
        return Err(
            "Automation đang chạy. Hãy dừng tiến trình trước khi dọn Chrome profiles.".to_string(),
        );
    }

    let profiles_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(CHROME_PROFILES_DIR);
    let (removed_files, removed_dirs, freed_bytes) = count_path_contents(&profiles_dir)?;

    if profiles_dir.exists() {
        fs::remove_dir_all(&profiles_dir)
            .map_err(|e| format!("Không xoá được {}: {}", profiles_dir.display(), e))?;
    }

    fs::create_dir_all(&profiles_dir)
        .map_err(|e| format!("Không tạo lại {}: {}", profiles_dir.display(), e))?;

    Ok(CleanupChromeProfilesResult {
        path: profiles_dir.display().to_string(),
        removed_files,
        removed_dirs,
        freed_bytes,
    })
}
