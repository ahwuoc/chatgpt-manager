use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use futures::stream::{self, StreamExt};
use serde_json::{json, Value};
use std::{collections::HashSet, fs, io::Write, sync::Arc};
use tauri::Emitter;
use tokio::sync::Mutex;
use wreq::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};

const CHECKOUT_URL: &str = "https://chatgpt.com/backend-api/payments/checkout";
const OUTPUT_FILE: &str = "data/results/01_stripe_checkout_links.jsonl";
const SUCCESS_EMAILS_FILE: &str = "data/success_emails.txt";
const DEFAULT_MAX_CONCURRENT: usize = 20;

fn load_success_emails() -> HashSet<String> {
    match fs::read_to_string(SUCCESS_EMAILS_FILE) {
        Ok(content) => content
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        Err(_) => HashSet::new(),
    }
}

pub async fn run(app: tauri::AppHandle, emails: Vec<String>, threads: u32) -> Result<()> {
    macro_rules! println {
        ($($arg:tt)*) => {
            let msg = format!($($arg)*);
            let _ = app.emit("automation-log", msg);
        };
    }

    println!("🚀 ĐANG TẠO STRIPE CHECKOUT LINK TRỰC TIẾP TRÊN TAURI ENGINE...");
    std::fs::create_dir_all("data/results").ok();

    if emails.is_empty() {
        let _ = fs::write(OUTPUT_FILE, "");
    } else {
        if let Ok(content) = fs::read_to_string(OUTPUT_FILE) {
            let filtered_lines: Vec<String> = content
                .lines()
                .filter(|line| {
                    if let Ok(v) = serde_json::from_str::<Value>(line) {
                        if let Some(email) = v.get("email").and_then(|e| e.as_str()) {
                            return !emails.contains(&email.to_string());
                        }
                    }
                    true
                })
                .map(|s| s.to_string())
                .collect();
            let _ = fs::write(OUTPUT_FILE, filtered_lines.join("\n") + "\n");
        }
    }

    let registry = Arc::new(load_success_emails());
    println!(
        "📋 Đã tải registry: {} acc đã reg trial từ {}",
        registry.len(),
        SUCCESS_EMAILS_FILE
    );

    let mut accounts: Vec<(String, String, String)> = Vec::new();

    let content = match fs::read_to_string("data/access_tokens.txt") {
        Ok(c) => c,
        Err(_) => {
            println!("❌ Không tìm thấy access_tokens.txt");
            return Ok(());
        }
    };

    let trimmed = content.trim();
    if trimmed.starts_with('{') {
        if let Ok(json_val) = serde_json::from_str::<Value>(trimmed) {
            push_from_json(&json_val, &mut accounts, &emails);
        }
    } else {
        for line in trimmed.lines().filter(|l| !l.trim().is_empty()) {
            let line_trimmed = line.trim();
            if line_trimmed.starts_with('{') {
                if let Ok(json_val) = serde_json::from_str::<Value>(line_trimmed) {
                    push_from_json(&json_val, &mut accounts, &emails);
                }
            } else if let Some((email, access_token)) = line_trimmed.split_once('|') {
                let email = email.trim().to_string();
                if !emails.is_empty() && !emails.contains(&email) {
                    continue;
                }
                if let Some(acc_id) = get_account_id(access_token) {
                    accounts.push((email, access_token.to_string(), acc_id));
                } else {
                    println!("⚠️  Không tách được account_id cho {}, bỏ qua", email);
                }
            }
        }
    }

    if accounts.is_empty() {
        println!("❌ Không có tài khoản nào được chọn hoặc được xử lý!");
        return Ok(());
    }

    {
        let before = accounts.len();
        accounts.retain(|(email, _, _)| !registry.contains(email));
        let skipped = before - accounts.len();
        if skipped > 0 {
            println!(
                "⏭️  Bỏ qua {} acc đã reg trial thành công trước đó",
                skipped
            );
        }
    }

    if accounts.is_empty() {
        println!("✅ Tất cả tài khoản đã được reg trial rồi, không có gì để làm!");
        return Ok(());
    }

    let concurrency_limit = (threads as usize).max(1).min(DEFAULT_MAX_CONCURRENT);
    println!(
        "📊 Sẽ xử lý {} tài khoản — tối đa {} luồng song song...",
        accounts.len(),
        concurrency_limit
    );

    let mut client_builder = wreq::Client::builder();
    if let Some(proxy) = load_proxy(&app) {
        client_builder = client_builder.proxy(proxy);
    }
    let client = Arc::new(client_builder.build()?);

    let output_mutex = Arc::new(Mutex::new(()));

    stream::iter(accounts)
        .map(|(email, access_token, account_id)| {
            let client = Arc::clone(&client);
            let output_mutex = Arc::clone(&output_mutex);
            let app_clone = app.clone();

            async move {
                let _ = app_clone.emit(
                    "automation-log",
                    format!("👤 [{}] Bắt đầu lấy Stripe Link...", email),
                );

                let result = process_account(
                    app_clone.clone(),
                    &client,
                    &email,
                    &access_token,
                    &account_id,
                    &output_mutex,
                )
                .await;

                match result {
                    Ok(true) => {
                        let _ = app_clone.emit(
                            "automation-log",
                            format!(
                                "⏳ [{email}] Đã lấy xong link Stripe. Sẵn sàng cho Bước 3. ✅"
                            ),
                        );
                    }
                    Ok(false) => {
                        let _ = app_clone
                            .emit("automation-log", format!("❌ [{email}] Xử lý thất bại"));
                    }
                    Err(e) => {
                        let _ = app_clone.emit("automation-log", format!("❌ [{email}] Lỗi: {e}"));
                    }
                }
            }
        })
        .buffer_unordered(concurrency_limit)
        .collect::<Vec<_>>()
        .await;

    println!("\n✨ Hoàn tất tất cả tài khoản của Bước 2!");
    Ok(())
}

async fn process_account(
    app: tauri::AppHandle,
    client: &wreq::Client,
    email: &str,
    access_token: &str,
    account_id: &str,
    output_mutex: &Arc<Mutex<()>>,
) -> Result<bool> {
    macro_rules! println {
        ($($arg:tt)*) => {
            let msg = format!($($arg)*);
            let _ = app.emit("automation-log", msg);
        };
    }

    let headers = build_headers(access_token, account_id)?;
    let body = build_body();

    let resp = match client
        .post(CHECKOUT_URL)
        .headers(headers)
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            println!("❌ [{email}] Lỗi request: {e}");
            return Ok(false);
        }
    };

    let status = resp.status();

    if !status.is_success() {
        let body_text = resp.text().await.unwrap_or_default();
        println!("❌ [{email}] API lỗi HTTP {status}: {body_text}");
        return Ok(false);
    }

    let res_json: Value = resp.json().await?;

    let Some(checkout_url) = res_json.get("url").and_then(|v| v.as_str()) else {
        println!("❌ [{email}] Response không có url: {res_json:?}");
        return Ok(false);
    };

    println!("✅ [{email}] Thành công lấy link Stripe!");

    {
        let _lock = output_mutex.lock().await;
        save_payment_link(email, checkout_url)?;
    }

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    Ok(true)
}

fn push_from_json(
    json_val: &Value,
    accounts: &mut Vec<(String, String, String)>,
    emails: &Vec<String>,
) {
    let email = json_val
        .get("user")
        .and_then(|u| u.get("email"))
        .and_then(|e| e.as_str())
        .unwrap_or("unknown")
        .to_string();

    if !emails.is_empty() && !emails.contains(&email) {
        return;
    }

    let access_token = json_val
        .get("accessToken")
        .and_then(|t| t.as_str())
        .map(|t| t.to_string());

    let account_id = json_val
        .get("account")
        .and_then(|a| a.get("id"))
        .and_then(|i| i.as_str())
        .map(|i| i.to_string())
        .or_else(|| access_token.as_ref().and_then(|t| get_account_id(t)));

    if let (Some(token), Some(acc_id)) = (access_token, account_id) {
        accounts.push((email, token, acc_id));
    }
}

fn build_headers(access_token: &str, account_id: &str) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {access_token}"))?,
    );
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert("OpenAI-Account-Id", HeaderValue::from_str(account_id)?);
    headers.insert("Origin", HeaderValue::from_static("https://chatgpt.com"));
    headers.insert("Referer", HeaderValue::from_static("https://chatgpt.com/"));
    headers.insert(
        "User-Agent",
        HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
             AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/120.0.0.0 Safari/537.36",
        ),
    );
    Ok(headers)
}

fn build_body() -> Value {
    json!({
        "entry_point": "all_plans_pricing_modal",
        "plan_name": "chatgptplusplan",
        "billing_details": {
            "country": "US",
            "currency": "USD"
        },
        "promo_campaign": {
            "promo_campaign_id": "plus-1-month-free",
            "is_coupon_from_query_param": false
        },
        "checkout_ui_mode": "hosted",
        "cancel_url": "https://chatgpt.com/#pricing"
    })
}

fn save_payment_link(email: &str, checkout_url: &str) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(OUTPUT_FILE)?;

    let data = json!({
        "email": email,
        "checkout_url": checkout_url,
        "created_at": chrono::Local::now().to_rfc3339()
    });

    writeln!(file, "{}", data.to_string())?;
    Ok(())
}

fn get_account_id(jwt: &str) -> Option<String> {
    let payload = jwt.split('.').nth(1)?;
    let mut padded = payload.replace('-', "+").replace('_', "/");
    while padded.len() % 4 != 0 {
        padded.push('=');
    }
    let decoded = general_purpose::STANDARD.decode(padded).ok()?;
    let parsed: Value = serde_json::from_slice(&decoded).ok()?;
    parsed
        .get("https://api.openai.com/auth")?
        .get("chatgpt_account_id")?
        .as_str()
        .map(str::to_string)
}

fn load_proxy(app: &tauri::AppHandle) -> Option<wreq::Proxy> {
    macro_rules! println {
        ($($arg:tt)*) => {
            let msg = format!($($arg)*);
            let _ = app.emit("automation-log", msg);
        };
    }

    let content = fs::read_to_string("data/proxies.txt").ok()?;
    let line = content.lines().find(|l| !l.trim().is_empty())?;
    let line = line.trim();

    let parts: Vec<&str> = line.split(':').collect();
    if parts.len() == 4 {
        let (ip, port, user, pass) = (parts[0], parts[1], parts[2], parts[3]);
        let proxy_url = format!("http://{}:{}", ip, port);
        match wreq::Proxy::all(&proxy_url) {
            Ok(proxy) => {
                println!("🔒 Proxy: {}:{} (user: {})", ip, port, user);
                Some(proxy.basic_auth(user, pass))
            }
            Err(e) => {
                println!("⚠️  Lỗi proxy URL {proxy_url}: {e}");
                None
            }
        }
    } else if parts.len() == 2 {
        let (ip, port) = (parts[0], parts[1]);
        let proxy_url = format!("http://{}:{}", ip, port);
        match wreq::Proxy::all(&proxy_url) {
            Ok(proxy) => {
                println!("🔒 Proxy không mật khẩu: {}:{}", ip, port);
                Some(proxy)
            }
            Err(e) => {
                println!("⚠️  Lỗi proxy URL {proxy_url}: {e}");
                None
            }
        }
    } else {
        match wreq::Proxy::all(line) {
            Ok(proxy) => {
                println!("🔒 Proxy từ URL: {}", line);
                Some(proxy)
            }
            Err(e) => {
                println!("⚠️  Lỗi proxy {line}: {e}");
                None
            }
        }
    }
}
