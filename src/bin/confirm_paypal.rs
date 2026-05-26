use anyhow::Result;
use serde_json::Value;
use std::{
    fs,
    io::{self, Write},
    sync::Arc,
};
use wreq::header::{CONTENT_TYPE, ORIGIN, REFERER};

const STRIPE_PK: &str = "pk_live_51HOrSwC6h1nxGoI3lTAgRjYVrz4dU3fVOabyCcKR3pbEJguCVAlqCxdxCUvoRh1XWwRacViovU3kLKvpkjh7IqkW00iXQsjo3n";
const PAYMENT_LINKS_FILE: &str = "results/01_stripe_checkout_links.jsonl";
const PAYPAL_LINKS_FILE: &str = "results/02_paypal_approve_links.txt";

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let batch_size: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(5);

    println!(
        "🚀 PAYMENT PAYPAL — CHẾ ĐỘ BATCH ({} BROWSER CÁCH LY CÙNG LÚC)",
        batch_size
    );
    std::fs::create_dir_all("results").ok();

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
            entries.push(v);
        } else {
            println!("⚠️ Dòng {} không parse được JSON", i + 1);
        }
    }

    if entries.is_empty() {
        println!("❌ Không có link nào trong {}", PAYMENT_LINKS_FILE);
        return Ok(());
    }

    println!("📋 Tìm thấy {} checkout link(s)", entries.len());

    let client = Arc::new(
        wreq::Client::builder()
            .emulation(wreq_util::Emulation::Chrome124)
            .build()?,
    );

    let mut success_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(PAYPAL_LINKS_FILE)?;

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

            fetch_tasks.push(tokio::spawn(async move {
                let cs_id = checkout_url
                    .split("/pay/")
                    .last()
                    .and_then(|s| s.split('#').next());

                if let Some(id) = cs_id {
                    get_paypal_url(&client, &email, id).await
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

    println!("\n✨ HOÀN TẤT TẤT CẢ BATCH!");
    Ok(())
}

async fn get_paypal_url(
    client: &wreq::Client,
    email: &str,
    cs_id: &str,
) -> Option<(String, String)> {
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
