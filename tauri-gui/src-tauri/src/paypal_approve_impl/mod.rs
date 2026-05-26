pub mod flow;
pub mod page;
pub mod steps;
pub mod utils;

use anyhow::{anyhow, Result};
use chaser_oxide::{Browser, BrowserConfig, ChaserPage};
use futures::StreamExt;
use std::collections::HashSet;
use std::fs;
use std::time::Duration;
use tauri::Emitter;

use crate::paths::{PAYPAL_LINKS_FILE, SUCCESS_FILE};
use crate::paypal_approve_impl::flow::run_approval_flow;

async fn process_link(
    app: tauri::AppHandle,
    email: &str,
    url: &str,
    slot: usize,
    max_cols: usize,
) -> Result<()> {
    macro_rules! println {
        ($($arg:tt)*) => {
            let msg = format!($($arg)*);
            std::println!("{}", msg);
            let _ = app.emit("automation-log", msg);
        };
    }

    println!("🚀 [Slot {}] {} | Bắt đầu duyệt PayPal...", slot, email);

    let window_w: u32 = 320;
    let window_h: u32 = 540;
    let (window_x, window_y) =
        crate::utils::tiled_window_position(slot, max_cols, window_w, window_h);
    let window_class = crate::utils::browser_window_class("", email, slot);
    let profile_dir = crate::utils::browser_profile_dir(&app, "", email)?;

    println!("  🗂️ [{}] Profile dir: {}", email, profile_dir.display());

    let builder = BrowserConfig::builder()
        .with_head()
        .window_size(window_w, window_h)
        .arg(format!("--window-position={},{}", window_x, window_y))
        .arg(format!("--class={}", window_class))
        .arg("--ozone-platform=x11")
        .arg("--no-first-run")
        .arg("--hide-crash-restore-bubble")
        .arg("--disable-features=InfiniteSessionRestore")
        .arg("--disable-web-security")
        .arg("--disable-site-isolation-trials")
        .user_data_dir(profile_dir.to_string_lossy().into_owned());
    let config = crate::us_browser_proxy::apply_to_browser_builder(builder, &app, "PAYPAL", email)?
        .build()
        .map_err(|e| anyhow!("Lỗi cấu hình browser: {}", e))?;

    let (browser, mut handler) = Browser::launch(config).await?;
    tokio::spawn(async move { while handler.next().await.is_some() {} });
    crate::utils::force_tile_window(&window_class, window_x, window_y, window_w, window_h).await;

    let page = browser.new_page("about:blank").await?;
    crate::us_browser_proxy::authenticate_page(&page, &app, "PAYPAL", email).await?;
    let chaser = ChaserPage::new(page);
    crate::utils::apply_fingerprint_profile(&chaser, &app, "PAYPAL", email).await?;

    println!("  🌐 [{}] Mở URL PayPal…", email);
    if let Err(e) = chaser.goto(url).await {
        println!(
            "  ⚠️ [{}] Mở PayPal bị timeout/lỗi: {}. Không đóng browser, kiểm tra trang hiện tại...",
            email, e
        );
    }
    tokio::time::sleep(Duration::from_secs(5)).await;

    let current_url = match chaser.url().await {
        Ok(Some(current_url)) => current_url,
        Ok(None) => String::new(),
        Err(e) => {
            println!(
                "  ⚠️ [{}] Không đọc được URL sau khi mở PayPal: {:?}.",
                email, e
            );
            String::new()
        }
    };

    if current_url.is_empty() || current_url == "about:blank" {
        println!(
            "  ⚠️ [{}] Tab vẫn chưa vào PayPal, dùng fallback window.location...",
            email
        );
        let url_json = serde_json::to_string(url).map_err(|e| anyhow!(e.to_string()))?;
        let script = format!(
            r#"(() => {{
                window.location.href = {};
                return 'NAVIGATING';
            }})()"#,
            url_json
        );
        if let Err(e) = chaser.raw_page().evaluate(script.as_str()).await {
            println!("  ⚠️ [{}] Fallback mở URL PayPal lỗi: {:?}.", email, e);
        }
        tokio::time::sleep(Duration::from_secs(8)).await;
    }

    let flow_res = run_approval_flow(app.clone(), &chaser, email).await;
    match &flow_res {
        Ok(_) => {
            println!(
                "  ✅ [{}] Luồng tự động hoàn tất thành công. Đóng trình duyệt...",
                email
            );
            println!("  ✅ [{}] Đóng trình duyệt.", email);
            drop(browser);
        }
        Err(e) => {
            println!("  ⚠️ [{}] Luồng tự động gặp lỗi: {}.", email, e);
            println!(
                "  🚨 [{}] Đã kích hoạt chế độ giữ trình duyệt MỞ trong 15 phút để anh có thể kiểm tra hoặc tự điền tay/resend!",
                email
            );
            tokio::time::sleep(Duration::from_secs(900)).await;
            drop(browser);
        }
    }
    flow_res
}

pub async fn run(app: tauri::AppHandle, emails: Vec<String>, threads: u32) -> Result<()> {
    macro_rules! println {
        ($($arg:tt)*) => {
            let msg = format!($($arg)*);
            std::println!("{}", msg);
            let _ = app.emit("automation-log", msg);
        };
    }

    println!("🚀 PAYPAL APPROVE — DUYỆT TỰ ĐỘNG NATIVE TRÊN TAURI ENGINE");

    let content = fs::read_to_string(PAYPAL_LINKS_FILE)
        .map_err(|_| anyhow!("❌ Không tìm thấy {}", PAYPAL_LINKS_FILE))?;

    let success_emails: HashSet<String> = fs::read_to_string(SUCCESS_FILE)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect();

    let mut seen_batch_emails = HashSet::new();
    let mut duplicate_batch_emails = HashSet::new();

    let links: Vec<(String, String)> = content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let (email, url) = line.split_once('|')?;
            let email = email.trim();
            if success_emails.contains(email) {
                return None;
            }
            if !emails.is_empty() && !emails.contains(&email.to_string()) {
                return None;
            }
            Some((email.to_string(), url.to_string()))
        })
        .filter_map(|(email, url)| {
            if seen_batch_emails.insert(email.clone()) {
                Some((email, url))
            } else {
                duplicate_batch_emails.insert(email);
                None
            }
        })
        .collect();

    if links.is_empty() {
        println!("❌ Không có link nào cần xử lý hoặc khớp bộ lọc!");
        return Ok(());
    }

    if !duplicate_batch_emails.is_empty() {
        println!(
            "⚠️ Bỏ qua {} email bị trùng profile trong batch hiện tại.",
            duplicate_batch_emails.len()
        );
    }

    let concurrent_limit = (threads as usize).max(1);
    println!(
        "📋 Tìm thấy {} link PayPal — chạy tối đa {} luồng song song.",
        links.len(),
        concurrent_limit
    );

    futures::stream::iter(links.into_iter().enumerate())
        .for_each_concurrent(concurrent_limit, |(idx, (email, url))| {
            let app_clone = app.clone();
            async move {
                let slot = idx % concurrent_limit;
                if let Err(e) =
                    process_link(app_clone.clone(), &email, &url, slot, concurrent_limit).await
                {
                    let _ =
                        app_clone.emit("automation-log", format!("❌ Lỗi slot {}: {}", slot, e));
                }
            }
        })
        .await;

    println!("\n✨ HOÀN TẤT BƯỚC 4!");
    Ok(())
}
