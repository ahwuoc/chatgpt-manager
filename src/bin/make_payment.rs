use anyhow::Result;
use base64::{Engine as _, engine::general_purpose};
use futures::stream::{self, StreamExt};
use serde_json::{Value, json};
use std::{collections::HashSet, fs, io::Write, sync::Arc};
use tokio::sync::Mutex;
use wreq::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};

const CHECKOUT_URL: &str = "https://chatgpt.com/backend-api/payments/checkout";
const OUTPUT_FILE: &str = "results/01_stripe_checkout_links.jsonl";
const SUCCESS_EMAILS_FILE: &str = "success_emails.txt";
const MAX_CONCURRENT: usize = 20;

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

// ─────────────────────────────────────────────────────────────
// Main
// ─────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    println!(
        "🚀 ĐANG CHẠY SCRIPT (tối đa {} tab song song)...",
        MAX_CONCURRENT
    );

    std::fs::create_dir_all("results").ok();
    let _ = fs::write(OUTPUT_FILE, "");

    // Load danh sách email đã reg trial thành công do user nhập thủ công
    let registry = Arc::new(load_success_emails());
    println!(
        "📋 Đã tải registry: {} acc đã reg trial từ {}",
        registry.len(),
        SUCCESS_EMAILS_FILE
    );

    // ── Parse accounts ──────────────────────────────────────
    let args: Vec<String> = std::env::args().collect();
    let mut accounts: Vec<(String, String, String)> = Vec::new();

    if args.len() > 1 {
        let input = &args[1];
        parse_single_input(input, &mut accounts);
    } else {
        let content = match fs::read_to_string("access_tokens.txt") {
            Ok(c) => c,
            Err(_) => {
                println!("❌ Không tìm thấy access_tokens.txt");
                return Ok(());
            }
        };
        parse_file_content(&content, &mut accounts);
    }

    if accounts.is_empty() {
        println!("❌ Không có tài khoản nào được xử lý!");
        return Ok(());
    }

    // ── Lọc bỏ acc đã reg trial ─────────────────────────────
    {
        let before = accounts.len();
        accounts.retain(|(email, _, _)| !registry.contains(email));
        let skipped = before - accounts.len();
        if skipped > 0 {
            println!("⏭️  Bỏ qua {} acc đã reg trial trước đó", skipped);
        }
    }

    if accounts.is_empty() {
        println!("✅ Tất cả tài khoản đã được reg trial rồi, không có gì để làm!");
        return Ok(());
    }

    println!("📊 Sẽ xử lý {} tài khoản...", accounts.len());

    // ── Build HTTP client ────────────────────────────────────
    let mut client_builder = wreq::Client::builder();
    if let Some(proxy) = load_proxy() {
        client_builder = client_builder.proxy(proxy);
    }
    let client = Arc::new(client_builder.build()?);

    let output_mutex = Arc::new(Mutex::new(()));

    stream::iter(accounts)
        .map(|(email, access_token, account_id)| {
            let client = Arc::clone(&client);
            let output_mutex = Arc::clone(&output_mutex);

            async move {
                println!("\n──────────────────────────────────────────────────");
                println!("👤 [{}] Bắt đầu xử lý...", email);

                let result =
                    process_account(&client, &email, &access_token, &account_id, &output_mutex)
                        .await;

                match result {
                    Ok(true) => {
                        println!("⏳ [{email}] Đã lấy xong link Stripe. Chờ xử lý ở bước PayPal.");
                    }
                    Ok(false) => {
                        println!("❌ [{email}] Xử lý thất bại");
                    }
                    Err(e) => {
                        println!("❌ [{email}] Lỗi: {e}");
                    }
                }
            }
        })
        .buffer_unordered(MAX_CONCURRENT)
        .collect::<Vec<_>>()
        .await;

    println!("\n✨ Hoàn tất tất cả tài khoản!");
    Ok(())
}

// ─────────────────────────────────────────────────────────────
// Xử lý 1 tài khoản — trả về Ok(true) nếu thành công
// ─────────────────────────────────────────────────────────────

async fn process_account(
    client: &wreq::Client,
    email: &str,
    access_token: &str,
    account_id: &str,
    output_mutex: &Arc<Mutex<()>>,
) -> Result<bool> {
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

    println!("✅ [{email}] Thành công:");
    println!("   {checkout_url}");

    // Ghi file an toàn dùng Mutex
    {
        let _lock = output_mutex.lock().await;
        save_payment_link(email, checkout_url)?;
    }

    // Delay nhỏ để tránh rate-limit
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    Ok(true)
}

// ─────────────────────────────────────────────────────────────
// Parse helpers
// ─────────────────────────────────────────────────────────────

fn parse_single_input(input: &str, accounts: &mut Vec<(String, String, String)>) {
    if let Ok(json_val) = serde_json::from_str::<Value>(input) {
        push_from_json(&json_val, accounts);
    } else if let Some((email, access_token)) = input.split_once('|') {
        if let Some(acc_id) = get_account_id(access_token) {
            accounts.push((email.to_string(), access_token.to_string(), acc_id));
        } else {
            println!("❌ Không tách được account_id từ token");
        }
    } else if let Some(acc_id) = get_account_id(input) {
        accounts.push(("unknown".to_string(), input.to_string(), acc_id));
    } else {
        println!("❌ Không thể phân tích chuỗi tham số truyền vào");
    }
}

fn parse_file_content(content: &str, accounts: &mut Vec<(String, String, String)>) {
    let trimmed = content.trim();
    if trimmed.starts_with('{') {
        if let Ok(json_val) = serde_json::from_str::<Value>(trimmed) {
            push_from_json(&json_val, accounts);
        } else {
            println!("❌ access_tokens.txt bắt đầu bằng '{{' nhưng không phải JSON hợp lệ");
        }
        return;
    }

    for line in trimmed.lines().filter(|l| !l.trim().is_empty()) {
        let line_trimmed = line.trim();
        if line_trimmed.starts_with('{') {
            if let Ok(json_val) = serde_json::from_str::<Value>(line_trimmed) {
                push_from_json(&json_val, accounts);
            }
        } else if let Some((email, access_token)) = line_trimmed.split_once('|') {
            if let Some(acc_id) = get_account_id(access_token) {
                accounts.push((email.to_string(), access_token.to_string(), acc_id));
            } else {
                println!("⚠️  Không tách được account_id cho {email}, bỏ qua");
            }
        } else if let Some(acc_id) = get_account_id(line_trimmed) {
            accounts.push(("unknown".to_string(), line_trimmed.to_string(), acc_id));
        }
    }
}

fn push_from_json(json_val: &Value, accounts: &mut Vec<(String, String, String)>) {
    let email = json_val
        .get("user")
        .and_then(|u| u.get("email"))
        .and_then(|e| e.as_str())
        .unwrap_or("unknown")
        .to_string();

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
    } else {
        println!("⚠️  JSON thiếu accessToken hoặc account_id, bỏ qua");
    }
}

// ─────────────────────────────────────────────────────────────
// HTTP helpers
// ─────────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────────
// JWT / Proxy helpers
// ─────────────────────────────────────────────────────────────

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

fn load_proxy() -> Option<wreq::Proxy> {
    let content = fs::read_to_string("proxies.txt").ok()?;
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
