use anyhow::Result;
use serde_json::Value;
use std::{fs, io::Write, sync::Arc};
use tauri::Emitter;
use wreq::header::{CONTENT_TYPE, ORIGIN, REFERER};

const STRIPE_PK: &str = "pk_live_51HOrSwC6h1nxGoI3lTAgRjYVrz4dU3fVOabyCcKR3pbEJguCVAlqCxdxCUvoRh1XWwRacViovU3kLKvpkjh7IqkW00iXQsjo3n";
const PAYMENT_LINKS_FILE: &str = "data/results/01_stripe_checkout_links.jsonl";
const PAYPAL_LINKS_FILE: &str = "data/results/02_paypal_approve_links.txt";

pub async fn run(app: tauri::AppHandle, emails: Vec<String>, threads: u32) -> Result<()> {
    macro_rules! println {
        ($($arg:tt)*) => {
            let msg = format!($($arg)*);
            let _ = app.emit("automation-log", msg);
        };
    }

    println!("🚀 CONFIRM PAYPAL — CHẠY TRỰC TIẾP TRÊN TAURI ENGINE");
    std::fs::create_dir_all("data/results").ok();

    if !emails.is_empty() {
        if let Ok(content) = fs::read_to_string(PAYPAL_LINKS_FILE) {
            let filtered_lines: Vec<String> = content
                .lines()
                .filter(|line| {
                    if let Some((email, _)) = line.split_once('|') {
                        return !emails.contains(&email.trim().to_string());
                    }
                    true
                })
                .map(|s| s.to_string())
                .collect();
            let _ = fs::write(PAYPAL_LINKS_FILE, filtered_lines.join("\n") + "\n");
        }
    }

    let content = match fs::read_to_string(PAYMENT_LINKS_FILE) {
        Ok(c) => c,
        Err(_) => {
            println!("❌ Không tìm thấy file {}", PAYMENT_LINKS_FILE);
            return Ok(());
        }
    };

    let mut entries = Vec::new();
    for (i, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            let email = v
                .get("email")
                .and_then(|e| e.as_str())
                .unwrap_or("")
                .to_string();
            // Filter by checkboxes if list is active
            if !emails.is_empty() && !emails.contains(&email) {
                continue;
            }
            entries.push(v);
        } else {
            println!("⚠️ Dòng {} không parse được JSON", i + 1);
        }
    }

    if entries.is_empty() {
        println!(
            "❌ Không có link nào cần xác nhận trong {}",
            PAYMENT_LINKS_FILE
        );
        return Ok(());
    }

    println!("📋 Tìm thấy {} checkout link(s) được chọn", entries.len());

    let client = Arc::new(
        wreq::Client::builder()
            .emulation(wreq_util::Emulation::Chrome124)
            .build()?,
    );

    let mut success_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(PAYPAL_LINKS_FILE)?;

    let batch_size = (threads as usize).max(1);
    let chunks: Vec<_> = entries.chunks(batch_size).collect();

    for (idx, chunk) in chunks.iter().enumerate() {
        println!("\n==================================================");
        println!(
            "📦 BATCH {}/{} ({} accounts)",
            idx + 1,
            chunks.len(),
            chunk.len()
        );
        println!("==================================================");

        let mut fetch_tasks = Vec::new();
        for entry in *chunk {
            let email = entry
                .get("email")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let checkout_url = entry
                .get("checkout_url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let client = Arc::clone(&client);
            let app_clone = app.clone();

            fetch_tasks.push(tokio::spawn(async move {
                let cs_id = checkout_url
                    .split("/pay/")
                    .last()
                    .and_then(|s| s.split('#').next());

                if let Some(id) = cs_id {
                    get_paypal_url(app_clone, &client, &email, id).await
                } else {
                    None
                }
            }));
        }

        let results = futures::future::join_all(fetch_tasks).await;

        let mut opened_count = 0;
        for res in results {
            if let Ok(Some((email, paypal_url))) = res {
                let _ = writeln!(success_file, "{}|{}", email, paypal_url);
                println!("  💾 Đã lấy và lưu link PayPal thành công cho: {}", email);
                opened_count += 1;
            }
        }

        if opened_count > 0 {
            println!(
                "\n✅ Đã lưu thành công {} link PayPal vào file {}.",
                opened_count, PAYPAL_LINKS_FILE
            );
        } else {
            println!("❌ Batch này không có link PayPal nào thành công.");
        }
    }

    println!("\n✨ HOÀN TẤT TẤT CẢ BATCH CỦA BƯỚC 3!");
    Ok(())
}

async fn get_paypal_url(
    app: tauri::AppHandle,
    client: &wreq::Client,
    email: &str,
    cs_id: &str,
) -> Option<(String, String)> {
    macro_rules! println {
        ($($arg:tt)*) => {
            let msg = format!($($arg)*);
            let _ = app.emit("automation-log", msg);
        };
    }

    let params = [
        ("key", STRIPE_PK),
        ("eid", "NA"),
        ("expected_amount", "0"),
        ("expected_payment_method_type", "paypal"),
        ("payment_method_data[type]", "paypal"),
        (
            "payment_method_data[billing_details][address][country]",
            "US",
        ),
        (
            "payment_method_data[billing_details][address][postal_code]",
            "10001",
        ),
        ("return_url", "https://chatgpt.com/#pricing"),
        ("consent[terms_of_service]", "accepted"),
    ];

    let mut body_str = String::new();
    for (k, v) in params.iter() {
        if !body_str.is_empty() {
            body_str.push('&');
        }
        body_str.push_str(&urlencoding::encode(k));
        body_str.push('=');
        body_str.push_str(&urlencoding::encode(v));
    }

    let api_url = format!("https://api.stripe.com/v1/payment_pages/{}/confirm", cs_id);

    let resp = match client
        .post(&api_url)
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(REFERER, "https://checkout.stripe.com/")
        .header(ORIGIN, "https://checkout.stripe.com")
        .body(body_str)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            println!("  ❌ [{}] Lỗi Request: {}", email, e);
            return None;
        }
    };

    let res_json: Value = resp.json().await.ok()?;

    if let Some(error) = res_json.get("error") {
        println!(
            "  ❌ [{}] Stripe Error: {}",
            email,
            error["message"].as_str().unwrap_or("Unknown")
        );
        return None;
    }

    let intent = res_json
        .get("setup_intent")
        .or_else(|| res_json.get("payment_intent"));

    let paypal_url = intent
        .and_then(|i| i.get("next_action"))
        .and_then(|na| na.get("redirect_to_url"))
        .and_then(|ru| ru.get("url"))
        .and_then(|u| u.as_str());

    if let Some(url) = paypal_url {
        let mut final_url = url.to_string();
        if final_url.contains('?') {
            final_url.push_str("&locale.x=en_US&country.x=US&landing_page=billing");
        } else {
            final_url.push_str("?locale.x=en_US&country.x=US&landing_page=billing");
        }

        println!("  ✅ [{}] Lấy link PayPal thành công", email);
        Some((email.to_string(), final_url))
    } else {
        println!("  ⚠️ [{}] Không tìm thấy link redirect", email);
        None
    }
}
