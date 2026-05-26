use crate::mail_otp;
use crate::otp;
use crate::paths::{ACCESS_TOKENS_FILE, PLUS_VERIFIED_REAL_FILE};
use futures::StreamExt;
use std::fs;
use tauri::{AppHandle, Emitter};

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlusMailScanResult {
    email: String,
    status: String,
    reason: String,
    mail_count: usize,
    matched_subject: Option<String>,
    matched_date: Option<String>,
}

fn text_has_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn is_chatgpt_plus_subscription_mail(msg: &otp::OTPMessage) -> bool {
    let subject = msg.subject.to_lowercase();
    let from = msg.from.to_lowercase();
    let body = msg.message.as_deref().unwrap_or_default().to_lowercase();
    let combined = format!("{} {} {}", from, subject, body);

    let is_openai_mail = from.contains("openai")
        || subject.contains("chatgpt")
        || subject.contains("openai")
        || body.contains("the openai team")
        || body.contains("openai ·");
    let is_new_plan = text_has_any(
        &combined,
        &[
            "chatgpt - your new plan",
            "your new plan",
            "successfully subscribed to chatgpt plus",
            "you've successfully subscribed to chatgpt plus",
            "you’ve successfully subscribed to chatgpt plus",
        ],
    );
    let has_plus_plan = text_has_any(
        &combined,
        &[
            "chatgpt plus subscription",
            "chatgpt plus",
            "plus subscription",
        ],
    );
    let has_trial_or_payment_proof = text_has_any(
        &combined,
        &[
            "enjoy your first month free",
            "first month free",
            "discount: -$20",
            "discount: -$20.00",
            "total: $0",
            "total: $0.00",
            "payment method paypal",
            "order number: sub_",
            "manage your account: https://chatgpt.com/account/manage",
        ],
    );

    is_openai_mail && is_new_plan && has_plus_plan && has_trial_or_payment_proof
}

fn persist_plus_verified_real(newly_verified: &[String]) {
    if newly_verified.is_empty() {
        return;
    }

    if let Some(parent) = std::path::Path::new(PLUS_VERIFIED_REAL_FILE).parent() {
        let _ = fs::create_dir_all(parent);
    }

    let mut existing = std::collections::HashSet::new();
    if let Ok(content) = fs::read_to_string(PLUS_VERIFIED_REAL_FILE) {
        for line in content.lines() {
            let email = line.trim();
            if !email.is_empty() {
                existing.insert(email.to_string());
            }
        }
    }

    for email in newly_verified {
        existing.insert(email.clone());
    }

    let mut final_list: Vec<String> = existing.into_iter().collect();
    final_list.sort();
    let _ = fs::write(PLUS_VERIFIED_REAL_FILE, final_list.join("\n"));
}

fn classify_mail_fetch_error(error: &str) -> (&'static str, String) {
    let lower = error.to_lowercase();
    let looks_like_server_busy = [
        "timeout",
        "timed out",
        "429",
        "too many",
        "rate",
        "overload",
        "busy",
        "502",
        "503",
        "504",
        "gateway",
        "server",
        "connection",
        "connect",
        "request",
        "network",
        "dns",
        "html",
    ]
    .iter()
    .any(|needle| lower.contains(needle));

    if looks_like_server_busy {
        return (
            "Mail Server Busy",
            "Server mail/API đang quá tải hoặc request bị nghẽn, thử quét lại sau.".to_string(),
        );
    }

    if lower.contains("parse json") || lower.contains("json") {
        return (
            "Mail Error",
            "API mail trả phản hồi không đúng định dạng JSON, chưa kết luận được acc này."
                .to_string(),
        );
    }

    (
        "Mail Error",
        "Lỗi request mail/API, chưa kết luận được acc này.".to_string(),
    )
}
#[tauri::command]
pub async fn scan_plus_status(
    app: AppHandle,
    emails: Vec<String>,
) -> Result<Vec<(String, String)>, String> {
    let app_clone = app.clone();
    macro_rules! log {
        ($($arg:tt)*) => {
            let _ = app_clone.emit("automation-log", format!($($arg)*));
        };
    }

    log!("🔍 BẮT ĐẦU QUÉT BẢN QUYỀN PLUS/TRIAL THỰC TẾ (KHÔNG DÙNG TRÌNH DUYỆT)...");

    if emails.is_empty() {
        log!("❌ Không có tài khoản nào được chọn để quét!");
        return Ok(Vec::new());
    }

    log!(
        "📋 Chuẩn bị quét {} tài khoản. Đang đọc Access Tokens...",
        emails.len()
    );

    let mut tokens_map = std::collections::HashMap::new();
    if let Ok(content) = fs::read_to_string(ACCESS_TOKENS_FILE) {
        for line in content.lines() {
            let line = line.trim();
            if !line.is_empty() {
                if let Some((email, token)) = line.split_once('|') {
                    tokens_map.insert(email.trim().to_string(), token.trim().to_string());
                }
            }
        }
    }
    let client = match wreq::Client::builder()
        .emulation(wreq_util::Emulation::Chrome124)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            log!("❌ Lỗi khởi tạo wreq Client: {}", e);
            return Err(e.to_string());
        }
    };

    let mut results = Vec::new();
    let mut newly_verified = Vec::new();

    for email in emails {
        log!("⏳ Đang check: {} ...", email);
        let token = match tokens_map.get(&email) {
            Some(t) => t,
            None => {
                log!(
                    "⚠️ [{}] Không tìm thấy Access Token trong access_tokens.txt!",
                    email
                );
                results.push((email.clone(), "No Token".to_string()));
                continue;
            }
        };

        let resp_res = client
            .get("https://chatgpt.com/backend-api/accounts/check/v4-2023-04-27")
            .header("Authorization", &format!("Bearer {}", token))
            .header("Accept", "application/json")
            .send()
            .await;

        match resp_res {
            Ok(resp) => {
                let status_code = resp.status().as_u16();
                let resp_text = resp.text().await.unwrap_or_default();
                if resp_text.contains("plan_type") {
                    let is_plus = resp_text.contains("\"plan_type\":\"plus\"")
                        || resp_text.contains("\"plan_type\": \"plus\"")
                        || resp_text.contains("\"is_paid_subscription_active\":true")
                        || resp_text.contains("\"is_paid_subscription_active\": true");

                    if is_plus {
                        log!("✅ [{}] 👉 BẢN QUYỀN PLUS THẬT (PLAN: PLUS)!", email);
                        results.push((email.clone(), "Plus Real".to_string()));
                        newly_verified.push(email);
                    } else {
                        log!("❌ [{}] 👉 TÀI KHOẢN FREE (PLAN: FREE/NONE)!", email);
                        results.push((email.clone(), "Free".to_string()));
                    }
                } else if resp_text.contains("token_expired") || status_code == 401 {
                    log!("⚠️ [{}] 👉 Token hết hạn hoặc không hợp lệ!", email);
                    results.push((email.clone(), "Token Expired".to_string()));
                } else {
                    let preview = if resp_text.len() > 100 {
                        &resp_text[..100]
                    } else {
                        &resp_text
                    };
                    log!(
                        "❓ [{}] 👉 Lỗi phản hồi API (Status {}): {}...",
                        email,
                        status_code,
                        preview
                    );
                    results.push((email.clone(), "API Error".to_string()));
                }
            }
            Err(e) => {
                log!("❌ [{}] Lỗi gửi request wreq: {}", email, e);
                results.push((email.clone(), "wreq Error".to_string()));
            }
        }

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    let plus_real_count = results
        .iter()
        .filter(|(_, status)| status == "Plus Real")
        .count();
    let free_count = results
        .iter()
        .filter(|(_, status)| status == "Free")
        .count();
    let no_token_count = results
        .iter()
        .filter(|(_, status)| status == "No Token")
        .count();
    let token_expired_count = results
        .iter()
        .filter(|(_, status)| status == "Token Expired")
        .count();
    let api_error_count = results
        .len()
        .saturating_sub(plus_real_count + free_count + no_token_count + token_expired_count);

    if !newly_verified.is_empty() {
        let file_path = "data/results/05_plus_verified_real.txt";
        if let Some(parent) = std::path::Path::new(file_path).parent() {
            let _ = fs::create_dir_all(parent);
        }

        // Read existing and merge uniquely
        let mut existing = std::collections::HashSet::new();
        if let Ok(content) = fs::read_to_string(file_path) {
            for line in content.lines() {
                if !line.trim().is_empty() {
                    existing.insert(line.trim().to_string());
                }
            }
        }

        for email in &newly_verified {
            existing.insert(email.clone());
        }

        let mut final_list: Vec<String> = existing.into_iter().collect();
        final_list.sort();
        let _ = fs::write(file_path, final_list.join("\n"));
        log!(
            "💾 Đã cập nhật thêm {} acc Plus Trial Thật vào file: {} ✅",
            newly_verified.len(),
            file_path
        );
    }

    log!(
        "✨ HOÀN TẤT QUÉT TRẠNG THÁI PLUS! Plus thật: {}/{} acc | Free: {} | No token: {} | Token lỗi: {} | API lỗi: {}",
        plus_real_count,
        results.len(),
        free_count,
        no_token_count,
        token_expired_count,
        api_error_count
    );
    Ok(results)
}

#[tauri::command]
pub async fn scan_plus_mail_status(
    app: AppHandle,
    emails: Vec<String>,
) -> Result<Vec<PlusMailScanResult>, String> {
    let app_clone = app.clone();
    macro_rules! log {
        ($($arg:tt)*) => {
            let _ = app_clone.emit("automation-log", format!($($arg)*));
        };
    }

    log!("📬 BẮT ĐẦU QUÉT MAIL XÁC NHẬN CHATGPT PLUS/TRIAL...");

    if emails.is_empty() {
        log!("❌ Không có tài khoản nào được chọn để quét mail!");
        return Ok(Vec::new());
    }

    let account_map = mail_otp::read_mail_account_map();
    let total = emails.len();
    let concurrency = total.clamp(1, 8);
    log!(
        "⚡ Quét mail song song {} luồng cho {} tài khoản...",
        concurrency,
        total
    );

    let mut results: Vec<PlusMailScanResult> = futures::stream::iter(
        emails
            .into_iter()
            .enumerate()
            .map(|(index, email)| (index, email.trim().to_lowercase())),
    )
    .map(|(index, email_key)| {
        let app_item = app_clone.clone();
        let account = account_map.get(&email_key).cloned();
        async move {
            let _ = app_item.emit(
                "automation-log",
                format!("📨 [{}/{}] Đang quét mail OpenAI: {} ...", index + 1, total, email_key),
            );

            let account = match account {
                Some(account) => account,
                None => {
                    let reason = "Không tìm thấy email này trong data/accounts_list.txt nên không gọi được Microsoft Graph.".to_string();
                    let _ = app_item.emit(
                        "automation-log",
                        format!("⚠️ [{}] {}", email_key, reason),
                    );
                    return PlusMailScanResult {
                        email: email_key,
                        status: "No Mail Config".to_string(),
                        reason,
                        mail_count: 0,
                        matched_subject: None,
                        matched_date: None,
                    };
                }
            };

            let otp_service = otp::OTPService::new();
            match otp_service
                .fetch_messages(
                    &account.email,
                    &account.password,
                    if account.refresh_token.is_empty() {
                        None
                    } else {
                        Some(account.refresh_token.as_str())
                    },
                    if account.client_id.is_empty() {
                        None
                    } else {
                        Some(account.client_id.as_str())
                    },
                )
                .await
            {
                Ok(messages) => {
                    let plus_mail = messages
                        .iter()
                        .find(|msg| is_chatgpt_plus_subscription_mail(msg));

                    if let Some(msg) = plus_mail {
                        let reason = format!(
                            "Tìm thấy mail xác nhận ChatGPT Plus/trial: '{}' ({})",
                            msg.subject, msg.date
                        );
                        let _ = app_item.emit(
                            "automation-log",
                            format!(
                                "✅ [{}] {}",
                                email_key, reason
                            ),
                        );
                        PlusMailScanResult {
                            email: email_key,
                            status: "Plus Mail Real".to_string(),
                            reason,
                            mail_count: messages.len(),
                            matched_subject: Some(msg.subject.clone()),
                            matched_date: Some(msg.date.clone()),
                        }
                    } else {
                        let reason = format!(
                            "Không tìm thấy mail xác nhận Plus Trial/OpenAI new plan trong {} mail mới nhất.",
                            messages.len()
                        );
                        let _ = app_item.emit(
                            "automation-log",
                            format!("❌ [{}] {}", email_key, reason),
                        );
                        PlusMailScanResult {
                            email: email_key,
                            status: "No Plus Mail".to_string(),
                            reason,
                            mail_count: messages.len(),
                            matched_subject: None,
                            matched_date: None,
                        }
                    }
                }
                Err(e) => {
                    let (status, reason) = classify_mail_fetch_error(&e.to_string());
                    let _ = app_item.emit(
                        "automation-log",
                        format!("❌ [{}] {}", email_key, reason),
                    );
                    PlusMailScanResult {
                        email: email_key,
                        status: status.to_string(),
                        reason,
                        mail_count: 0,
                        matched_subject: None,
                        matched_date: None,
                    }
                }
            }
        }
    })
    .buffer_unordered(concurrency)
    .collect()
    .await;

    results.sort_by(|a, b| a.email.cmp(&b.email));
    let newly_verified: Vec<String> = results
        .iter()
        .filter(|item| item.status == "Plus Mail Real")
        .map(|item| item.email.clone())
        .collect();

    persist_plus_verified_real(&newly_verified);
    if !newly_verified.is_empty() {
        log!(
            "💾 Đã cập nhật {} acc Plus Trial Thật từ mail vào file: {} ✅",
            newly_verified.len(),
            PLUS_VERIFIED_REAL_FILE
        );
    }

    let plus_real_count = results
        .iter()
        .filter(|item| item.status == "Plus Mail Real")
        .count();
    let no_plus_mail_count = results
        .iter()
        .filter(|item| item.status == "No Plus Mail")
        .count();
    let no_config_count = results
        .iter()
        .filter(|item| item.status == "No Mail Config")
        .count();
    let server_busy_count = results
        .iter()
        .filter(|item| item.status == "Mail Server Busy")
        .count();
    let mail_error_count = results
        .iter()
        .filter(|item| item.status == "Mail Error")
        .count();

    log!(
        "✨ HOÀN TẤT QUÉT MAIL PLUS! Plus mail thật: {}/{} acc | Không có mail Plus: {} | Thiếu config mail: {} | Server/API quá tải: {} | Lỗi request khác: {}",
        plus_real_count,
        results.len(),
        no_plus_mail_count,
        no_config_count,
        server_busy_count,
        mail_error_count
    );

    Ok(results)
}
