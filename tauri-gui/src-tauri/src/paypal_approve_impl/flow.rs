use anyhow::{anyhow, Result};
use chaser_oxide::ChaserPage;
use std::time::Duration;

use crate::paths::{FLOW_TIMEOUT_SECS, POLL_INTERVAL_SECS};
use crate::paypal_approve_impl::page::PaypalPage;
use crate::paypal_approve_impl::steps::{
    refill_phone_and_submit, step_consent, step_create_account, step_fill_card, step_fill_email,
    step_handle_otp, OtpResult,
};
use crate::paypal_approve_impl::utils::{
    mark_trial_plus_fail, mark_trial_plus_success,
};

pub struct FlowState {
    pub create_account_clicked: bool,
    pub email_filled: bool,
    pub card_filled: bool,
    pub card_submit_wait_cycles: u8,
    pub consent_clicked: bool,
    pub sms_order_id: Option<String>,
    pub otp_attempts: u8,
    pub otp_challenge_seen: bool,
}

pub async fn run_approval_flow(
    app: tauri::AppHandle,
    chaser: &ChaserPage,
    email: &str,
) -> Result<()> {
    let raw = chaser.raw_page();
    let pp = PaypalPage::new(raw, email, app.clone());

    pp.log("Phân tích trang PayPal...");

    let mut state = FlowState {
        create_account_clicked: false,
        email_filled: false,
        card_filled: false,
        card_submit_wait_cycles: 0,
        consent_clicked: false,
        sms_order_id: None,
        otp_attempts: 0,
        otp_challenge_seen: false,
    };
    let mut last_logged_url = String::new();
    let mut waiting_for_captcha = false;

    let flow = async {
        loop {
            let url = match chaser.url().await {
                Ok(Some(u)) => u,
                Ok(None) => String::new(),
                Err(e) => {
                    pp.log("❌ Trình duyệt hoặc tab đã bị đóng! Dừng tiến trình.");
                    return Err(anyhow!("Trình duyệt bị đóng: {:?}", e));
                }
            };
            pp.inject_debug_overlay().await;
            pp.inject_api_sniffer().await;
            pp.extract_and_save_api_logs().await;

            if url != last_logged_url {
                pp.log(&format!("Chuyển URL: {}…", &url[..url.len().min(80)]));
                last_logged_url = url.clone();
            }

            if url.contains("chatgpt.com") && url.contains("redirect_status=succeeded") {
                pp.log(
                    "🎉 Phát hiện URL chuyển hướng đăng ký thành công (redirect_status=succeeded)!",
                );
                mark_trial_plus_success(email);
                pp.log("Đã đánh dấu acc Reg Trial Plus Success ✅");
                tokio::time::sleep(Duration::from_secs(5)).await;
                return Ok(());
            }

            if url.contains("chatgpt.com")
                && (url.contains("redirect_status=failed") || url.contains("redirect_status=fail"))
            {
                pp.log("❌ Phát hiện URL báo lỗi thanh toán (redirect_status=failed/fail)!");
                mark_trial_plus_fail(email);
                pp.log("Đã đánh dấu acc Reg Trial Plus Fail ❌");
                tokio::time::sleep(Duration::from_secs(5)).await;
                return Err(anyhow!("Giao dịch thất bại (redirect_status=failed/fail)"));
            }

            let has_captcha = pp.is_security_challenge_page().await;
            if has_captcha {
                if !waiting_for_captcha {
                    pp.log("⚠️ Đang ở Security Challenge/CAPTCHA, tạm dừng auto-fill cho tới khi challenge biến mất.");
                    waiting_for_captcha = true;
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
            if waiting_for_captcha {
                waiting_for_captcha = false;
                pp.log("✅ Security Challenge đã biến mất, tiếp tục flow...");
            }

            let is_pay_page = url.contains("paypal") || !url.is_empty();
            let waiting_for_otp_challenge = if is_pay_page {
                pp.is_waiting_for_otp_challenge().await
            } else {
                false
            };

            if waiting_for_otp_challenge {
                state.card_filled = true;
                state.card_submit_wait_cycles = 0;
                if !state.otp_challenge_seen {
                    pp.log(
                        "📱 PayPal đang ở bước OTP/challenge → giữ nguyên form, chỉ chờ/xử lý mã.",
                    );
                    state.otp_challenge_seen = true;
                }
            } else if state.otp_challenge_seen {
                state.otp_challenge_seen = false;
            }

            if is_pay_page && !waiting_for_otp_challenge && !state.create_account_clicked {
                if step_create_account(&pp).await {
                    state.create_account_clicked = true;
                    continue;
                }
            }

            if !waiting_for_otp_challenge && state.email_filled {
                let email_field_empty = pp.eval_bool(r#"(() => {
                    const el = document.getElementById('login_email') ||
                               document.getElementById('email') ||
                               document.getElementById('onboardingFlowEmail') ||
                               document.querySelector('input[type="email"]');
                    return !!el && (!el.value || el.value.length < 3);
                })()"#).await;
                if email_field_empty {
                    pp.log("⚠️ Phát hiện ô nhập Email trống nhưng state.email_filled = true. Reset về false để fill lại...");
                    state.email_filled = false;
                }
            }

            if is_pay_page && !waiting_for_otp_challenge && !state.email_filled {
                if step_fill_email(&pp).await {
                    state.email_filled = true;
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    continue;
                }
            }

            if !waiting_for_otp_challenge
                && state.email_filled
                && pp.is_create_account_email_form().await
            {
                pp.log("⚠️ Form email vẫn hiển thị sau khi fill → reset và thử lại...");
                state.email_filled = false;
            }

            let has_card_form = pp.wait_for_id("cardNumber", 1).await;
            let has_add_card_btn = pp.eval_bool(r#"(() => {
                    return !!(
                        document.querySelector('[data-testid="add-fi-link"] button') ||
                        document.evaluate(
                            "//button[contains(.,'Add a debit or credit card') or contains(.,'Add a card')]",
                            document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null
                        ).singleNodeValue
                    );
                })()"#).await;

            if !waiting_for_otp_challenge
                && state.card_filled
                && is_pay_page
                && (has_card_form || has_add_card_btn)
            {
                if pp.is_security_challenge_page().await {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
                if state.card_submit_wait_cycles < 12 {
                    state.card_submit_wait_cycles += 1;
                    pp.log("Đã submit thẻ, đang chờ trang xử lý/render bước tiếp theo...");
                } else {
                    pp.log("Sau khi submit quá lâu vẫn còn form thẻ → reset state và thử lại.");
                    state.card_filled = false;
                    state.card_submit_wait_cycles = 0;
                }
            }

            if is_pay_page && !waiting_for_otp_challenge && !state.card_filled {
                if has_card_form || has_add_card_btn {
                    if let Some(sms_id) = step_fill_card(&pp, &url).await {
                        pp.log("Submit thẻ → Đang xử lý... 💳");
                        state.card_filled = true;
                        state.sms_order_id = if sms_id.is_empty() {
                            None
                        } else {
                            Some(sms_id)
                        };
                        state.card_submit_wait_cycles = 0;
                    } else {
                        pp.log("⚠️ Chưa submit được form thẻ. Kiểm tra cấu hình phone rồi thử lại sau 30s...");
                        tokio::time::sleep(Duration::from_secs(30)).await;
                        continue;
                    }
                }
            }

            // ─── OTP Handling ───
            if state.card_filled {
                let sms_id = state.sms_order_id.clone().unwrap_or_default();
                match step_handle_otp(&pp, &sms_id, &mut state.otp_attempts).await {
                    OtpResult::Filled => {
                        pp.log("OTP đã điền thành công ✅");
                        tokio::time::sleep(Duration::from_secs(3)).await;
                    }
                    OtpResult::Resent => {
                        pp.log("Đã resend OTP, chờ vòng tiếp...");
                        continue;
                    }
                    OtpResult::NeedNewNumber => {
                        pp.log("Đổi số điện thoại và re-submit...");
                        if let Some(new_id) = refill_phone_and_submit(&pp).await {
                            state.sms_order_id = if new_id.is_empty() {
                                None
                            } else {
                                Some(new_id)
                            };
                            if state.sms_order_id.is_some() {
                                pp.log("Re-submit với số mới, chờ OTP modal...");
                            } else {
                                pp.log("Re-submit với phone thủ công, chờ bước tiếp theo...");
                            }
                        }
                        tokio::time::sleep(Duration::from_secs(3)).await;
                        continue;
                    }
                    OtpResult::NoModal => {
                        // No OTP modal, continue normal flow
                    }
                }
            }

            // Only check consent AFTER card has been filled
            if state.card_filled && !state.consent_clicked {
                if step_consent(&pp, &url).await {
                    pp.log("Nhấn 'Agree and Continue' → chờ ChatGPT trả redirect_status...");
                    state.consent_clicked = true;
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
            }

            if state.card_filled && !url.is_empty() && !url.contains("paypal") {
                pp.log("Đã rời PayPal, tiếp tục chờ redirect_status từ ChatGPT...");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }

            tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
        }
    };

    match tokio::time::timeout(Duration::from_secs(FLOW_TIMEOUT_SECS), flow).await {
        Ok(result) => result,
        Err(_) => {
            pp.log(&format!(
                "Timeout ({}s) — không hoàn thành được flow.",
                FLOW_TIMEOUT_SECS
            ));
            Err(anyhow!(
                "Timeout ({}s) khi chờ PayPal/ChatGPT trả kết quả cuối.",
                FLOW_TIMEOUT_SECS
            ))
        }
    }
}
