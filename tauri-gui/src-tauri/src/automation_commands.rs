use crate::app_state::AppState;
use crate::paths::{ACCESS_TOKENS_FILE, PAYPAL_LINKS_FILE, SUCCESS_FILE};
use crate::{auth, confirm_paypal_impl, make_payment_impl, paypal_approve_impl};
use anyhow::Result;
use std::fs;
use tauri::{AppHandle, Emitter, State};

fn has_valid_token(email: &str) -> bool {
    fs::read_to_string(ACCESS_TOKENS_FILE)
        .ok()
        .map(|content| {
            content.lines().any(|line| {
                let line = line.trim();
                let Some((line_email, token)) = line.split_once('|') else {
                    return false;
                };
                line_email.trim() == email
                    && token.trim().starts_with("eyJ")
                    && token.trim().split('.').count() == 3
            })
        })
        .unwrap_or(false)
}

fn has_paypal_link(email: &str) -> bool {
    fs::read_to_string(PAYPAL_LINKS_FILE)
        .ok()
        .map(|content| {
            content.lines().any(|line| {
                let Some((line_email, _url)) = line.trim().split_once('|') else {
                    return false;
                };
                line_email.trim() == email
            })
        })
        .unwrap_or(false)
}

fn has_success(email: &str) -> bool {
    fs::read_to_string(SUCCESS_FILE)
        .ok()
        .map(|content| content.lines().any(|line| line.trim() == email))
        .unwrap_or(false)
}

#[tauri::command]
pub async fn start_automation(
    app: AppHandle,
    state: State<'_, AppState>,
    mode: String,
    emails: Vec<String>,
    threads: u32,
) -> Result<(), String> {
    let mut task_lock = state.running_task.lock().unwrap();
    if task_lock.is_some() {
        return Err("Tiến trình automation đã đang chạy rồi!".to_string());
    }

    let app_clone = app.clone();
    let mode_clone = mode.clone();
    let emails_clone = emails.clone();

    let handle = tokio::spawn(async move {
        let _ = app_clone.emit("automation-status", "running");

        let run_result = match mode_clone.as_str() {
            "auth" => auth::run(app_clone.clone(), emails_clone, threads).await,
            "make_payment" => {
                let res =
                    make_payment_impl::run(app_clone.clone(), emails_clone.clone(), threads).await;
                if res.is_ok() {
                    confirm_paypal_impl::run(app_clone.clone(), emails_clone, threads).await
                } else {
                    res
                }
            }
            "confirm_paypal" => {
                confirm_paypal_impl::run(app_clone.clone(), emails_clone, threads).await
            }
            "paypal_approve" => {
                paypal_approve_impl::run(app_clone.clone(), emails_clone, threads).await
            }
            "auto_all" => {
                if threads <= 1 {
                    let total = emails_clone.len();
                    let _ = app_clone.emit(
                        "automation-log",
                        format!(
                            "🚦 [AUTO PIPELINE] Chế độ tuần tự: chạy trọn gói từng account ({} tài khoản).",
                            total
                        ),
                    );

                    for (index, email) in emails_clone.iter().enumerate() {
                        let email_list = vec![email.clone()];
                        let _ = app_clone.emit(
                            "automation-log",
                            format!(
                                "➡️ [AUTO PIPELINE] Account {}/{}: {}",
                                index + 1,
                                total,
                                email
                            ),
                        );

                        if has_success(email) {
                            let _ = app_clone.emit(
                                "automation-log",
                                format!("💡 [{}] Đã Success. Bỏ qua account này.", email),
                            );
                            continue;
                        }

                        if !has_valid_token(email) {
                            let _ = app_clone.emit(
                                "automation-log",
                                format!("🔄 [{}] Bước 1/3: Đăng nhập & lấy Token...", email),
                            );
                            if let Err(e) =
                                auth::run(app_clone.clone(), email_list.clone(), 1).await
                            {
                                let _ = app_clone.emit(
                                    "automation-log",
                                    format!("⚠️ [{}] Login lỗi: {}", email, e),
                                );
                            }
                        } else {
                            let _ = app_clone.emit(
                                "automation-log",
                                format!("💡 [{}] Đã có Token. Bỏ qua Bước 1.", email),
                            );
                        }

                        if !has_paypal_link(email) {
                            let _ = app_clone.emit(
                                "automation-log",
                                format!(
                                    "🔄 [{}] Bước 2/3: Tạo Stripe link & lấy PayPal link...",
                                    email
                                ),
                            );
                            let payment_res =
                                make_payment_impl::run(app_clone.clone(), email_list.clone(), 1)
                                    .await;
                            if payment_res.is_ok() {
                                let _ = confirm_paypal_impl::run(
                                    app_clone.clone(),
                                    email_list.clone(),
                                    1,
                                )
                                .await;
                            } else if let Err(e) = payment_res {
                                let _ = app_clone.emit(
                                    "automation-log",
                                    format!("⚠️ [{}] Tạo link lỗi: {}", email, e),
                                );
                            }
                        } else {
                            let _ = app_clone.emit(
                                "automation-log",
                                format!("💡 [{}] Đã có PayPal link. Bỏ qua Bước 2.", email),
                            );
                        }

                        if !has_success(email) {
                            let _ = app_clone.emit(
                                "automation-log",
                                format!("🔄 [{}] Bước 3/3: Duyệt PayPal...", email),
                            );
                            if let Err(e) =
                                paypal_approve_impl::run(app_clone.clone(), email_list, 1).await
                            {
                                let _ = app_clone.emit(
                                    "automation-log",
                                    format!("⚠️ [{}] Duyệt PayPal lỗi: {}", email, e),
                                );
                            }
                        }
                    }

                    Ok(())
                } else {
                    use std::collections::HashSet;
                    let mut logged_in_emails = HashSet::new();
                    if let Ok(content) = fs::read_to_string(ACCESS_TOKENS_FILE) {
                        for line in content.lines() {
                            let line = line.trim();
                            if !line.is_empty() {
                                if let Some((email, token)) = line.split_once('|') {
                                    let email = email.trim().to_string();
                                    let token = token.trim();
                                    if token.starts_with("eyJ") && token.split('.').count() == 3 {
                                        logged_in_emails.insert(email);
                                    }
                                }
                            }
                        }
                    }

                    let login_emails: Vec<String> = emails_clone
                        .iter()
                        .filter(|email| !logged_in_emails.contains(*email))
                        .cloned()
                        .collect();

                    if login_emails.is_empty() {
                        let _ = app_clone.emit("automation-log", "💡 [AUTO PIPELINE] 1/3: Tất cả tài khoản đều đã Đăng nhập (Login OK). Bỏ qua Bước 1.");
                    } else {
                        let _ = app_clone.emit("automation-log", &format!("🔄 [AUTO PIPELINE] Bước 1/3: Đăng nhập & lấy Token cho {} tài khoản...", login_emails.len()));
                        let auth_res = auth::run(app_clone.clone(), login_emails, threads).await;
                        if auth_res.is_err() {
                            let _ = app_clone.emit("automation-log", "⚠️ [AUTO PIPELINE] Gặp lỗi ở Bước 1 (Login), tiếp tục chuyển sang các bước sau...");
                        }
                    }

                    // 2. Filter out emails that ALREADY have a PayPal link for Step 2
                    let mut has_paypal_link_emails = HashSet::new();
                    if let Ok(content) = fs::read_to_string(PAYPAL_LINKS_FILE) {
                        for line in content.lines() {
                            let line = line.trim();
                            if !line.is_empty() {
                                if let Some((email, _url)) = line.split_once('|') {
                                    has_paypal_link_emails.insert(email.trim().to_string());
                                }
                            }
                        }
                    }

                    let payment_emails: Vec<String> = emails_clone
                        .iter()
                        .filter(|email| !has_paypal_link_emails.contains(*email))
                        .cloned()
                        .collect();

                    if payment_emails.is_empty() {
                        let _ = app_clone.emit("automation-log", "💡 [AUTO PIPELINE] 2/3: Tất cả tài khoản đều đã có Link PayPal. Bỏ qua Bước 2.");
                    } else {
                        let _ = app_clone.emit("automation-log", &format!("🔄 [AUTO PIPELINE] Bước 2/3: Khởi tạo trang thanh toán & Lấy Paypal link cho {} tài khoản...", payment_emails.len()));
                        let payment_res = make_payment_impl::run(
                            app_clone.clone(),
                            payment_emails.clone(),
                            threads,
                        )
                        .await;
                        if payment_res.is_ok() {
                            let _ = confirm_paypal_impl::run(
                                app_clone.clone(),
                                payment_emails,
                                threads,
                            )
                            .await;
                        } else {
                            let _ = app_clone.emit("automation-log", "⚠️ [AUTO PIPELINE] Gặp lỗi ở Bước 2 (Tạo link), tiếp tục thử duyệt PayPal...");
                        }
                    }

                    // 3. Filter out emails that ALREADY have a successful payment for Step 3
                    let mut success_emails = HashSet::new();
                    if let Ok(content) = fs::read_to_string(SUCCESS_FILE) {
                        for line in content.lines() {
                            let email = line.trim();
                            if !email.is_empty() {
                                success_emails.insert(email.to_string());
                            }
                        }
                    }

                    let approve_emails: Vec<String> = emails_clone
                        .iter()
                        .filter(|email| !success_emails.contains(*email))
                        .cloned()
                        .collect();

                    if approve_emails.is_empty() {
                        let _ = app_clone.emit("automation-log", "💡 [AUTO PIPELINE] 3/3: Tất cả tài khoản đã thanh toán thành công (Success). Hoàn thành!");
                        Ok(())
                    } else {
                        let _ = app_clone.emit("automation-log", &format!("🔄 [AUTO PIPELINE] Bước 3/3: Tự động điền thẻ, nhận OTP & Duyệt PayPal cho {} tài khoản...", approve_emails.len()));
                        paypal_approve_impl::run(app_clone.clone(), approve_emails, threads).await
                    }
                }
            }
            _ => paypal_approve_impl::run(app_clone.clone(), emails_clone, threads).await,
        };

        match run_result {
            Ok(_) => {
                let _ = app_clone.emit("automation-log", "🎉 Tiến trình automation hoàn thành!");
            }
            Err(e) => {
                let _ = app_clone.emit("automation-log", format!("❌ Lỗi chạy tiến trình: {}", e));
            }
        }

        let _ = app_clone.emit("automation-status", "idle");
    });

    *task_lock = Some(handle);

    // Monitor exit of thread to clean up lock state
    let running_task_clone = state.running_task.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let mut lock = running_task_clone.lock().unwrap();
            if let Some(ref h) = *lock {
                if h.is_finished() {
                    *lock = None;
                    break;
                }
            } else {
                break;
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn stop_automation(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let mut task_lock = state.running_task.lock().unwrap();
    if let Some(handle) = task_lock.take() {
        handle.abort();
        let _ = app.emit(
            "automation-log",
            "⏹ ĐÃ DỪNG KHẨN CẤP TIẾN TRÌNH THÀNH CÔNG!",
        );
        let _ = app.emit("automation-status", "idle");
        Ok(())
    } else {
        Err("Không có tiến trình nào đang chạy!".to_string())
    }
}
