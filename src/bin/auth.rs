#[path = "../otp.rs"]
mod otp;

use anyhow::{Result, anyhow};
use chaser_oxide::{Browser, BrowserConfig, ChaserPage};
use fake::Fake;
use fake::faker::name::en::Name;
use futures::StreamExt;
use otp::OTPService;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Account {
    email: String,
    password: String,
    #[serde(alias = "session_token")]
    session_token: Option<String>,
    #[serde(alias = "account_id")]
    account_id: Option<String>,
}

async fn wait_for_element(
    page: &chaser_oxide::Page,
    selector: &str,
    timeout_secs: u64,
) -> Result<chaser_oxide::Element> {
    let start = std::time::Instant::now();
    loop {
        if let Ok(el) = page.find_element(selector).await {
            return Ok(el);
        }
        if start.elapsed().as_secs() > timeout_secs {
            return Err(anyhow!("Timeout chờ đợi phần tử: {}", selector));
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

async fn run_login(account: Account, slot_index: usize) -> Result<()> {
    let start = std::time::Instant::now();
    let email = account.email.clone();

    println!(
        "🌐 [{}] Khởi động trình duyệt chaser-oxide (Stealth, Slot: {})...",
        email, slot_index
    );

    let width: u32 = 500;
    let height: u32 = 450;

    let x = 50 + (slot_index as u32) * (width + 15);
    let y = 100;
    let window_pos_arg = format!("--window-position={},{}", x, y);
    let user_data_dir = format!(
        "./chrome_profiles/{}",
        email.replace('@', "_").replace('.', "_")
    );

    let config = BrowserConfig::builder()
        .with_head()
        .window_size(width, height)
        .arg(window_pos_arg)
        .user_data_dir(user_data_dir)
        .build()
        .map_err(|e| anyhow!("Lỗi cấu hình chaser-oxide: {}", e))?;

    let (browser, mut handler) = Browser::launch(config).await?;
    tokio::spawn(async move { while let Some(_) = handler.next().await {} });

    let page = browser.new_page("about:blank").await?;
    let chaser = ChaserPage::new(page);
    chaser.apply_native_profile().await?;
    chaser.goto("https://chatgpt.com/auth/login").await?;
    tokio::time::sleep(Duration::from_secs(6)).await;

    println!("⏳ [{}] Chờ trang đăng nhập...", email);

    let raw = chaser.raw_page();

    let email_input = wait_for_element(
        raw,
        "input[type='email'], input[autocomplete='username']",
        30,
    )
    .await?;
    email_input.click().await?;
    tokio::time::sleep(Duration::from_millis(300)).await;

    println!("📧 [{}] Nhập email...", email);
    for c in email.chars() {
        email_input.type_str(&c.to_string()).await?;
        tokio::time::sleep(Duration::from_millis(80)).await;
    }

    tokio::time::sleep(Duration::from_millis(400)).await;
    email_input.press_key("Enter").await?;
    tokio::time::sleep(Duration::from_secs(5)).await;

    let otp_service = OTPService::new();
    let mut logged_in = false;

    for attempt in 1..=6 {
        // 0. Kiểm tra nếu bị chặn "Too many attempts" hoặc "Too many tries"
        if let Ok(res) = raw.evaluate("document.body.innerText").await {
            if let Ok(text) = res.into_value::<String>() {
                if text.contains("Too many attempts")
                    || text.contains("Too many tries")
                    || text.contains("max_check_attempts")
                {
                    println!(
                        "❌ [{}] Đăng nhập thất bại: Bị OpenAI chặn do thử quá nhiều lần (Too many attempts)!",
                        email
                    );
                    break;
                }
            }
        }

        let current_url = chaser.url().await.unwrap_or(None).unwrap_or_default();

        if current_url.contains("/about-you") {
            let fake_name: String = Name().fake();
            let fake_age: u32 = (22..45).fake();

            println!(
                "📝 [{}] Phát hiện trang hoàn tất thông tin cá nhân (/about-you). Đang điền tự động (Tên: {}, Tuổi: {})...",
                email, fake_name, fake_age
            );

            let fill_script = format!(
                r#"
                (() => {{
                    const inputs = Array.from(document.querySelectorAll('input')).filter(i => i.type !== 'hidden');
                    if (inputs.length >= 2) {{
                        const nameInput = inputs[0];
                        const ageInput = inputs[1];
                        
                        nameInput.value = "{}";
                        nameInput.dispatchEvent(new Event('input', {{ bubbles: true }}));
                        
                        ageInput.value = "{}";
                        ageInput.dispatchEvent(new Event('input', {{ bubbles: true }}));
                        
                        const btn = document.querySelector('button[type="submit"]') || Array.from(document.querySelectorAll('button')).find(b => b.textContent.includes('Finish'));
                        if (btn) {{
                            btn.click();
                            return 'SUCCESS';
                        }}
                    }}
                    return 'INPUTS_NOT_FOUND';
                }})()
                "#,
                fake_name, fake_age
            );

            let _ = chaser.evaluate(&fill_script).await;
            tokio::time::sleep(Duration::from_secs(6)).await;
            continue;
        }

        // 2. Trường hợp đã đăng nhập thành công vào ChatGPT chính thức
        if current_url.contains("chatgpt.com") && !current_url.contains("auth") {
            logged_in = true;
            break;
        }

        // 3. Tiến hành quét OTP và điền
        println!("🔑 [{}] Đang quét OTP lần {}...", email, attempt);
        let otp_res = otp_service
            .fetch_latest_otp(
                &account.email,
                &account.password,
                account.session_token.as_deref(),
                account.account_id.as_deref(),
            )
            .await;

        match otp_res {
            Ok(Some(otp)) => {
                println!("✅ [{}] Tìm thấy OTP: {}", email, otp);

                if let Ok(otp_input) = raw
                    .find_element("input[type='text'], input[inputmode='numeric']")
                    .await
                {
                    // An toàn kép: Chỉ nhập nếu URL hiện tại không phải là about-you
                    let check_url = chaser.url().await.unwrap_or(None).unwrap_or_default();
                    if check_url.contains("/about-you") {
                        continue;
                    }

                    // XÓA SẠCH MÃ OTP CŨ ĐANG BỊ KẸT TRONG Ô NHẬP LIỆU
                    let _ = raw.evaluate(r#"
                        (() => {
                            const el = document.querySelector("input[type='text'], input[inputmode='numeric']");
                            if (el) {
                                el.value = '';
                                el.dispatchEvent(new Event('input', { bubbles: true }));
                            }
                        })()
                    "#).await;
                    tokio::time::sleep(Duration::from_millis(200)).await;

                    otp_input.click().await?;
                    tokio::time::sleep(Duration::from_millis(300)).await;

                    for c in otp.chars() {
                        otp_input.type_str(&c.to_string()).await?;
                        tokio::time::sleep(Duration::from_millis(120)).await;
                    }

                    tokio::time::sleep(Duration::from_millis(400)).await;
                    otp_input.press_key("Enter").await?;
                }

                tokio::time::sleep(Duration::from_secs(6)).await;
            }
            Ok(None) => {
                println!(
                    "⚠️ [{}] Chưa nhận được email OTP từ Dongvanfb, sẽ tiếp tục quét...",
                    email
                );
            }
            Err(e) => {
                println!(
                    "❌ [{}] Lỗi kết nối API OTP hoặc phân tích cú pháp: {:?}",
                    email, e
                );
            }
        }

        tokio::time::sleep(Duration::from_secs(4)).await;
    }

    if logged_in {
        println!(
            "🎉 [{}] ĐĂNG NHẬP THÀNH CÔNG trong {:.1}s!",
            email,
            start.elapsed().as_secs_f64()
        );

        tokio::time::sleep(Duration::from_secs(2)).await;

        let script = r#"
            (async () => {
                try {
                    const res = await fetch("https://chatgpt.com/api/auth/session");
                    const data = await res.json();
                    return data.accessToken || "";
                } catch (e) {
                    return "";
                }
            })()
        "#;

        match raw.evaluate(script).await {
            Ok(js_val) => {
                if let Ok(token) = js_val.into_value::<String>() {
                    if !token.is_empty() {
                        println!("🔑 [{}] Trích xuất Access Token thành công!", email);
                        match fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open("access_tokens.txt")
                        {
                            Ok(mut file) => {
                                if let Err(e) = writeln!(file, "{}|{}", email, token) {
                                    println!(
                                        "❌ [{}] Lỗi ghi file access_tokens.txt: {:?}",
                                        email, e
                                    );
                                }
                            }
                            Err(e) => {
                                println!("❌ [{}] Lỗi mở file access_tokens.txt: {:?}", email, e);
                            }
                        }
                    } else {
                        println!(
                            "⚠️ [{}] Trích xuất được session JSON nhưng accessToken bị trống.",
                            email
                        );
                    }
                } else {
                    println!(
                        "❌ [{}] Không thể chuyển đổi JsValue của session thành String.",
                        email
                    );
                }
            }
            Err(e) => {
                println!(
                    "❌ [{}] Lỗi thực thi JavaScript trích xuất token: {:?}",
                    email, e
                );
            }
        }
    } else {
        println!("❌ [{}] Đăng nhập thất bại.", email);
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let content = fs::read_to_string("accounts_list.txt")
        .map_err(|_| anyhow!("Không tìm thấy file accounts_list.txt"))?;

    let mut accounts: Vec<Account> = Vec::new();
    let trimmed = content.trim();

    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        if let Ok(list) = serde_json::from_str::<Vec<Account>>(trimmed) {
            accounts = list;
        }
    } else {
        let args: Vec<String> = std::env::args().collect();
        let target_email = args.get(1).map(|s| s.trim().to_string());

        for line in trimmed.lines().filter(|l| !l.trim().is_empty()) {
            let parts: Vec<&str> = line.trim().split('|').collect();
            if parts.len() >= 2 {
                let email = parts[0].trim().to_string();
                if let Some(ref target) = target_email {
                    if &email != target {
                        continue;
                    }
                }
                accounts.push(Account {
                    email,
                    password: parts[1].trim().to_string(),
                    session_token: parts.get(2).map(|s| s.trim().to_string()),
                    account_id: parts.get(3).map(|s| s.trim().to_string()),
                });
            }
        }
    }

    if accounts.is_empty() {
        println!("❌ Danh sách tài khoản trống!");
        return Ok(());
    }

    println!(
        "🚀 Khởi chạy đăng nhập với chaser-oxide cho {} tài khoản (Đa luồng, tối đa 3 tài khoản chạy song song)...",
        accounts.len()
    );

    let concurrency_limit = 3;
    let mut stream = futures::stream::iter(accounts.into_iter().enumerate())
        .map(|(index, acc)| async move {
            let email = acc.email.clone();
            if index > 0 {
                let delay = (index % concurrency_limit) as u64 * 3;
                tokio::time::sleep(Duration::from_secs(delay)).await;
            }
            let slot_index = index % concurrency_limit;
            if let Err(e) = run_login(acc, slot_index).await {
                println!("❌ Lỗi [{}]: {}", email, e);
            }
        })
        .buffer_unordered(concurrency_limit);

    while let Some(_) = stream.next().await {}

    println!("\n✨ HOÀN TẤT!");
    Ok(())
}
