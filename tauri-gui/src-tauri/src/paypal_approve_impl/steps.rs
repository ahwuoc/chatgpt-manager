use crate::paypal_approve_impl::page::PaypalPage;
use crate::paypal_approve_impl::utils::CHECKOUT_LOG_FILE;
use crate::sms_service::SmsService;
use std::fs;
use std::io::Write;
use std::time::Duration;

fn ensure_us_phone(raw: &str) -> String {
    let digits: String = raw.chars().filter(|ch| ch.is_ascii_digit()).collect();
    if digits.starts_with('1') && digits.len() == 11 {
        return digits[1..].to_string();
    }
    if digits.len() == 10 {
        return digits;
    }
    if digits.len() > 10 {
        return digits.chars().take(10).collect();
    }
    let tail: String = (0..7)
        .map(|_| char::from_digit(rand::random_range(0..10), 10).unwrap())
        .collect();
    format!("202{}", tail)
}

async fn submit_card_form(pp: &PaypalPage<'_>) -> bool {
    let submitted = pp
        .eval_str(
            r#"(() => {
        const btn = document.querySelector('button[data-testid="submit-button"]')
                 || document.querySelector('button[data-atomic-wait-intent="click_select_create_account_and_continue"]')
                 || document.querySelector('button[type="submit"]');
        if (!btn) return 'NOT_FOUND';
        if (btn.disabled) return 'DISABLED';
        btn.click();
        const isVisible = (el) => {
            if (!el) return false;
            const rect = el.getBoundingClientRect();
            const style = window.getComputedStyle(el);
            return rect.width > 0 &&
                   rect.height > 0 &&
                   style.display !== 'none' &&
                   style.visibility !== 'hidden';
        };
        const findEl = (doc, sel) => {
            if (!doc) return null;
            let el = doc.getElementById(sel) || doc.querySelector('[name="' + sel + '"]') || doc.querySelector(sel);
            if (el) return el;
            let frames = doc.querySelectorAll('iframe');
            for (let i = 0; i < frames.length; i++) {
                try { el = findEl(frames[i].contentDocument, sel); if (el) return el; } catch(e) {}
            }
            return null;
        };
        const criticalIds = [
            "cardNumber", "cardExpiry", "cardCvv",
            "firstName", "lastName", "billingLine1",
            "billingCity", "billingPostalCode", "phone",
            "dateOfBirth", "password", "email", "countryCode_0"
        ];
        const hasVisibleInvalid = criticalIds.some((id) => {
            const el = findEl(document, id);
            if (!el || !isVisible(el)) return false;
            return String(el.getAttribute('aria-invalid') || '').toLowerCase() === 'true';
        });
        if (hasVisibleInvalid) return 'CLICKED_WITH_ERRORS';
        return 'CLICKED';
    })()"#,
        )
        .await;

    if submitted == "CLICKED_WITH_ERRORS" {
        pp.log("⚠️ Bấm submit nhưng form vẫn còn lỗi đỏ (aria-invalid).");
    }

    submitted == "CLICKED"
}

async fn ensure_us_country_stable(pp: &PaypalPage<'_>) -> bool {
    let mut stable_hits = 0u8;
    for _ in 0..10 {
        let is_us = pp
            .eval_bool(
                r#"(() => {
                const select =
                    document.querySelector('select[data-testid="countrySelector"]') ||
                    document.getElementById('country') ||
                    document.querySelector('select[name="country"]');
                return !!select && String(select.value || '').trim() === 'US';
            })()"#,
            )
            .await;

        if is_us {
            stable_hits += 1;
            if stable_hits >= 2 {
                return true;
            }
        } else {
            stable_hits = 0;
            let _ = pp.select_us_country().await;
        }

        tokio::time::sleep(Duration::from_millis(450)).await;
    }

    false
}

async fn has_country_selector(pp: &PaypalPage<'_>) -> bool {
    pp.eval_bool(
        r#"(() => {
            const select =
                document.querySelector('select[data-testid="countrySelector"]') ||
                document.getElementById('country') ||
                document.querySelector('select[name="country"]');
            return !!select;
        })()"#,
    )
    .await
}

async fn get_fill_health(pp: &PaypalPage<'_>) -> (usize, bool, String) {
    let raw = pp
        .eval_str(
            r#"(() => {
            const isVisible = (el) => {
                if (!el) return false;
                const rect = el.getBoundingClientRect();
                const style = window.getComputedStyle(el);
                return rect.width > 0 &&
                       rect.height > 0 &&
                       style.display !== 'none' &&
                       style.visibility !== 'hidden';
            };

            const findEl = (doc, sel) => {
                if (!doc) return null;
                let el = doc.getElementById(sel) || doc.querySelector('[name="' + sel + '"]') || doc.querySelector(sel);
                if (el) return el;
                let frames = doc.querySelectorAll('iframe');
                for (let i = 0; i < frames.length; i++) {
                    try { el = findEl(frames[i].contentDocument, sel); if (el) return el; } catch(e) {}
                }
                return null;
            };

            const hasForcedFlow = !!(
                document.querySelector('[data-testid="forced-with-password-flow"]') ||
                findEl(document, 'password') ||
                findEl(document, 'input[type="password"]')
            );
            const baseRequiredText = [
                "cardNumber", "cardExpiry", "cardCvv",
                "firstName", "lastName", "billingLine1", "billingCity"
            ];
            const forcedOnlyText = ["email", "phone", "dateOfBirth", "password", "billingPostalCode"];
            const requiredText = hasForcedFlow
                ? Array.from(new Set([...baseRequiredText, ...forcedOnlyText]))
                : [...baseRequiredText, "billingPostalCode"];

            const requiredSelect = ["country", "nationality", "countryCode_0"];
            const missing = [];
            let visibleCount = 0;

            for (const id of requiredText) {
                const el = findEl(document, id);
                if (!el || !isVisible(el)) {
                    continue;
                }
                visibleCount += 1;
                const val = ("value" in el) ? String(el.value || "").trim() : "";
                if (!val) missing.push(id);
            }

            for (const id of requiredSelect) {
                const el = findEl(document, id);
                if (!el || !isVisible(el)) continue;
                visibleCount += 1;
                const val = ("value" in el) ? String(el.value || "").trim() : "";
                if (!val || val !== "US") missing.push(id);
            }

            return `VISIBLE:${visibleCount}|FORCED:${hasForcedFlow ? 1 : 0}|MISSING:${missing.join(",")}`;
        })()"#,
        )
        .await;

    let mut visible_count = 0usize;
    let mut forced_flow = false;
    let mut missing = String::new();

    for part in raw.split('|') {
        if let Some(v) = part.strip_prefix("VISIBLE:") {
            visible_count = v.parse::<usize>().unwrap_or(0);
        } else if let Some(v) = part.strip_prefix("FORCED:") {
            forced_flow = v == "1";
        } else if let Some(v) = part.strip_prefix("MISSING:") {
            missing = v.to_string();
        }
    }

    (visible_count, forced_flow, missing)
}

pub async fn step_create_account(pp: &PaypalPage<'_>) -> bool {
    if pp.has_email_input().await {
        return false;
    }
    if pp.is_initial_create_account_prompt().await {
        for selector in &[
            "form[data-testid='create-account-form'] button[data-atomic-wait-task='login_create_account']",
            "form[data-testid='create-account-form'] button[type='submit']",
            "button[data-atomic-wait-task='login_create_account']",
        ] {
            if pp.click(selector).await {
                pp.log("Form chọn ban đầu → nhấn 'Create an Account'");
                tokio::time::sleep(Duration::from_secs(2)).await;
                return true;
            }
        }
    }

    let clicked = pp
        .eval_str(
            r#"(() => {
        const snap = document.evaluate(
            "//button[.//span[text()='Create an Account'] or text()='Create an Account']",
            document, null, XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null
        );
        for (let i = 0; i < snap.snapshotLength; i++) {
            const btn = snap.snapshotItem(i);
            if (btn && !btn.disabled && !btn.className.includes('bg-primary')) {
                btn.click(); return 'YES';
            }
        }
        return 'NO';
    })()"#,
        )
        .await;

    if clicked == "YES" {
        pp.log("Bấm 'Create an Account' → chuyển Form 2");
        tokio::time::sleep(Duration::from_secs(2)).await;
        true
    } else {
        false
    }
}

pub async fn step_fill_email(pp: &PaypalPage<'_>) -> bool {
    if pp.is_initial_create_account_prompt().await || pp.wait_for_id("cardNumber", 1).await {
        return false;
    }

    let is_create_account_email_form = pp.is_create_account_email_form().await;
    if !is_create_account_email_form && !pp.has_email_input().await {
        return false;
    }

    if is_create_account_email_form {
        pp.log("Form 'Create a PayPal account' → điền email và Continue...");
    } else {
        pp.log("Trang điền Email → bắt đầu điền...");
    }

    if has_country_selector(pp).await {
        pp.log("Form email có country selector → chọn US trước khi điền...");
        pp.select_us_country().await;
        tokio::time::sleep(Duration::from_millis(900)).await;
        if !ensure_us_country_stable(pp).await {
            pp.log("⚠️ Chưa giữ ổn định country=US, chờ vòng sau để tránh fill sai quốc gia.");
            return false;
        }
    }

    let fake_email = crate::utils::gen_email();
    pp.log(&format!("Email sẽ điền: {}", fake_email));

    for sel in &[
        "login_email",
        "email",
        "onboardingFlowEmail",
        "input[type='email']",
    ] {
        pp.fill(sel, &fake_email).await;
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    tokio::time::sleep(Duration::from_secs(1)).await;
    let filled_ok = pp
        .eval_bool(&format!(
            r#"(() => {{
            const el = document.getElementById('login_email')
                    || document.getElementById('email')
                    || document.querySelector('input[type="email"]');
            return !!(el && el.value && el.value.length > 3);
        }})()"#
        ))
        .await;

    if !filled_ok {
        pp.log("⚠️ Email chưa được điền thành công vào input, thử lại vòng sau...");
        return false;
    }

    let result = pp
        .eval_str(
            r#"(() => {
        const btn = document.querySelector(
            "button[data-atomic-wait-intent='Continue_To_Payment'], \
             button[data-atomic-wait-intent='Submit_Email'], \
             button[data-atomic-wait-task='login_enter_email'], \
             button[data-atomic-wait-intent='click_select_create_account_and_continue'], \
             button[data-testid='continueButton'], \
             button[data-testid='submit-button'], \
             button.actionContinue, \
             button[type='submit']"
        );
        if (btn && !btn.disabled) { btn.click(); return 'CLICKED'; }
        const xp = document.evaluate(
            "//button[contains(.,'Continue') or contains(.,'Next')]",
            document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null
        );
        if (xp.singleNodeValue && !xp.singleNodeValue.disabled) {
            xp.singleNodeValue.click(); return 'CLICKED_XPATH';
        }
        return 'NOT_FOUND';
    })()"#,
        )
        .await;

    if result.starts_with("CLICKED") {
        pp.log("Nhấn Continue sau khi điền email ✅");
        true
    } else {
        false
    }
}

pub async fn step_fill_card(pp: &PaypalPage<'_>, url: &str) -> Option<String> {
    if pp.is_security_challenge_page().await {
        pp.log("⚠️ Đang ở Security Challenge/CAPTCHA, tạm dừng điền thẻ.");
        return None;
    }

    if !pp.wait_for_id("cardNumber", 1).await {
        let clicked = pp.eval_str(r#"(() => {
            const btn = document.querySelector('[data-testid="add-fi-link"] button')
                || document.evaluate(
                    "//button[contains(.,'Add a debit or credit card') or contains(.,'Add a card')]",
                    document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null
                ).singleNodeValue;
            if (btn && !btn.disabled) { btn.click(); return 'CLICKED_ADD_CARD'; }
            return 'NO';
        })()"#).await;
        if clicked == "CLICKED_ADD_CARD" {
            pp.log("Nhấn 'Add a debit or credit card' ➕");
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        return None;
    }

    pp.log("Tìm thấy #cardNumber → bắt đầu điền form thẻ 💳");
    pp.select_us_country().await;
    tokio::time::sleep(Duration::from_millis(1200)).await;
    if !ensure_us_country_stable(pp).await {
        pp.log("⚠️ Country chưa ổn định ở US, bỏ qua lượt fill này để tránh điền sai form.");
        return None;
    }

    if !pp.wait_for_id("cardNumber", 5).await {
        pp.log("Form thẻ đang render lại sau khi đổi sang US, đợi vòng sau...");
        return None;
    }
    if !pp.wait_for_id("firstName", 5).await || !pp.wait_for_id("billingPostalCode", 5).await {
        pp.log("Form US chưa render đủ field bắt buộc, thử lại vòng sau...");
        return None;
    }

    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(CHECKOUT_LOG_FILE)
    {
        let _ = writeln!(file, "{}|{}", pp.email, url);
    }

    // Check if card is already filled to avoid constant refilling
    let is_already_filled = pp.eval_bool(
        r#"(() => {
            let findEl = (doc, sel) => {
                if (!doc) return null;
                let el = doc.getElementById(sel) || doc.querySelector('[name="' + sel + '"]') || doc.querySelector(sel);
                if (el) return el;
                let frames = doc.querySelectorAll('iframe');
                for (let i = 0; i < frames.length; i++) {
                    try { el = findEl(frames[i].contentDocument, sel); if (el) return el; } catch(e) {}
                }
                return null;
            };
            let el = findEl(document, 'cardNumber') || findEl(document, 'input[autocomplete="cc-number"]');
            return !!(el && el.value && el.value.replace(/\s/g, '').length >= 15);
        })()"#
    ).await;

    let sms_order_id = String::new();

    if is_already_filled {
        pp.log("💡 Phát hiện form đã có dữ liệu thẻ. Bỏ qua bước điền và tiến hành Submit...");
    } else {
        let card_num = crate::utils::gen_visa_card();
        let card_expiry = "12 / 30";
        let cvv = crate::utils::gen_cvv();
        let (first, last_name, street, city, state, zip, _generated_phone, pass) =
            crate::utils::gen_random_billing_info().await;

        let sms_config = SmsService::load_config();
        let raw_phone = if !sms_config.manual_phone.trim().is_empty() {
            let p = sms_config.manual_phone.trim().to_string();
            pp.log(&format!(
                "📞 Sử dụng số điện thoại thủ công cấu hình: {}",
                p
            ));
            p
        } else {
            pp.log(&format!(
                "📞 Cấu hình trống → Sử dụng số ngẫu nhiên sinh ra: {}",
                _generated_phone
            ));
            _generated_phone
        };
        let phone = ensure_us_phone(&raw_phone);

        if phone.chars().filter(|ch| ch.is_ascii_digit()).count() < 10 {
            pp.log("❌ Phone không hợp lệ, hủy submit form thẻ.");
            return None;
        }

        pp.log(&format!("Thẻ: {} / {} / {}", card_num, card_expiry, cvv));

        let mut fields: Vec<(&str, String)> = vec![
            ("cardNumber", card_num.clone()),
            ("cardExpiry", card_expiry.to_string()),
            ("cardCvv", cvv.clone()),
            ("phoneType", "MOBILE".to_string()),
            ("nationality", "US".to_string()),
            ("countryCode_0", "US".to_string()),
            ("firstName", first.clone()),
            ("lastName", last_name.clone()),
            ("billingLine1", street.clone()),
            ("billingCity", city.clone()),
            ("billingState", state.clone()),
            ("billingPostalCode", zip.clone()),
            ("phone", phone.clone()),
            ("dateOfBirth", "12011977".to_string()),
            ("password", pass.clone()),
        ];

        if pp.eval_bool("!!(document.getElementById('email') || document.querySelector('input[name=\"email\"]'))").await {
            let fake_email = crate::utils::gen_email();
            fields.push(("email", fake_email));
        }

        for (id, val) in &fields {
            pp.fill(id, val).await;
            tokio::time::sleep(Duration::from_millis(150)).await;
        }

        if pp.is_security_challenge_page().await {
            pp.log(
                "⚠️ Security Challenge bật giữa lúc điền form, giữ nguyên state và chờ vòng sau.",
            );
            return None;
        }

        let (visible_count_1, forced_flow_1, mut missing) = get_fill_health(pp).await;
        if visible_count_1 < 8 {
            pp.log(&format!(
                "⚠️ Form đang render/chưa ổn định (chỉ thấy {} field bắt buộc). Chờ vòng sau...",
                visible_count_1
            ));
            return None;
        }

        if !missing.is_empty() {
            pp.log(&format!(
                "⚠️ Sau lượt fill 1 còn thiếu field: {} → thử fill lại.",
                missing
            ));
            for (id, val) in &fields {
                pp.fill(id, val).await;
                tokio::time::sleep(Duration::from_millis(220)).await;
            }

            if pp.is_security_challenge_page().await {
                pp.log("⚠️ Security Challenge xuất hiện sau lượt fill lại, chờ vòng sau.");
                return None;
            }

            let (visible_count_2, forced_flow_2, missing_after_refill) = get_fill_health(pp).await;
            missing = missing_after_refill;
            if visible_count_2 < 8 {
                pp.log(&format!(
                    "⚠️ Sau refill, form vẫn chưa ổn định ({} field). Chờ vòng sau...",
                    visible_count_2
                ));
                return None;
            }
            if !missing.is_empty() {
                pp.log(&format!(
                    "❌ Fill US chưa thành công, vẫn thiếu: {}. Chờ vòng sau thử lại.",
                    missing
                ));
                return None;
            }

            if forced_flow_2 && !ensure_us_country_stable(pp).await {
                pp.log("⚠️ Country bị lệch khỏi US sau refill, dừng để tránh submit sai.");
                return None;
            }
        } else if forced_flow_1 && !ensure_us_country_stable(pp).await {
            pp.log("⚠️ Country bị lệch khỏi US trước submit, dừng để tránh submit sai.");
            return None;
        }

        pp.log("Đã điền xong toàn bộ form US (và email nếu có) ✅");
    }

    tokio::time::sleep(Duration::from_secs(1)).await;
    if submit_card_form(pp).await {
        Some(sms_order_id)
    } else {
        if pp.is_security_challenge_page().await {
            pp.log("⚠️ Submit bị chặn bởi Security Challenge, chờ giải challenge rồi bot tự chạy tiếp.");
            return None;
        }
        if !pp.wait_for_id("cardNumber", 1).await && !pp.wait_for_id("firstName", 1).await {
            pp.log("ℹ️ Form thẻ đã biến mất sau khi fill, có thể đang chuyển bước/challenge. Tạm chờ vòng sau.");
            return Some(sms_order_id);
        }
        pp.log("⚠️ Không tìm thấy nút submit form thẻ.");
        None
    }
}

pub async fn step_consent(pp: &PaypalPage<'_>, url: &str) -> bool {
    if pp.wait_for_id("cardNumber", 1).await {
        return false;
    }

    if url.contains("/billingweb/review") {
        pp.log("Trang review PayPal → thử nhấn 'Agree and Continue'...");
    }

    for selector in &[
        "#consentButton",
        "button[data-testid='consentButton']",
        "button#consentButton",
    ] {
        if pp.click(selector).await {
            return true;
        }
    }

    let res = pp
        .eval_str(
            r#"(() => {
        const btn = document.getElementById("consentButton");
        if (btn && !btn.disabled) { btn.click(); return 'CLICKED_CONSENT'; }
        const dt = document.querySelector("button[data-testid='consentButton']");
        if (dt && !dt.disabled) { dt.click(); return 'CLICKED_CONSENT'; }
        const submit = document.querySelector(
            "button[data-testid='submit-button'], button[data-atomic-wait-intent='click_select_create_account_and_continue']"
        );
        if (submit && !submit.disabled) {
            const text = (submit.textContent || '').trim();
            if (/Agree and Continue|Agree & Create|Đồng ý và tiếp tục/i.test(text)) {
                submit.click(); return 'CLICKED_FINAL_SUBMIT';
            }
        }
        const xp = document.evaluate(
            "//button[contains(normalize-space(.),'Agree and Continue') or contains(normalize-space(.),'Agree & Create') or contains(normalize-space(.),'Đồng ý và tiếp tục')]",
            document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null
        );
        if (xp.singleNodeValue && !xp.singleNodeValue.disabled) {
            xp.singleNodeValue.click(); return 'CLICKED_CONSENT';
        }
        return 'NOT_FOUND';
    })()"#,
        )
        .await;

    res == "CLICKED_CONSENT" || res == "CLICKED_FINAL_SUBMIT"
}

// ─── OTP Handling ────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtpResult {
    NoModal,
    Filled,
    Resent,
    NeedNewNumber,
}

fn extract_6_digit_otp(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    for i in 0..=chars.len().saturating_sub(6) {
        if chars[i..i + 6].iter().all(|c| c.is_ascii_digit()) {
            let not_preceded = i == 0 || !chars[i - 1].is_ascii_digit();
            let not_followed = i + 6 == chars.len() || !chars[i + 6].is_ascii_digit();
            if not_preceded && not_followed {
                let code: String = chars[i..i + 6].iter().collect();
                return Some(code);
            }
        }
    }
    None
}

pub async fn step_handle_otp(
    pp: &PaypalPage<'_>,
    _sms_order_id: &str,
    otp_attempts: &mut u8,
) -> OtpResult {
    let has_exceed = pp
        .eval_bool(
            r#"(() => {
        return !!(
            document.querySelector('[data-testid="exceed-main"]') ||
            document.querySelector('[data-testid="primary-button-exceed"]')
        );
    })()"#,
        )
        .await;

    if has_exceed {
        if *otp_attempts == 0 {
            pp.log(
                "⚠️ PayPal yêu cầu số khác. Hãy đổi/nhập phone thủ công hoặc cập nhật cấu hình.",
            );
            *otp_attempts = 1;
        }
        pp.click("button[data-testid='primary-button-exceed']")
            .await;
        tokio::time::sleep(Duration::from_secs(2)).await;
        return OtpResult::NoModal;
    }

    let has_modal = pp
        .eval_bool(
            r#"(() => {
        return !!(
            document.getElementById('ci-ciBasic-0') ||
            document.querySelector('[data-testid="sca-confirm-multi-field"]') ||
            document.querySelector('[id*="ci-"]') ||
            document.querySelector('input[autocomplete="one-time-code"]')
        );
    })()"#,
        )
        .await;

    if !has_modal {
        *otp_attempts = 0;
        return OtpResult::NoModal;
    }

    let sms_config = SmsService::load_config();
    let relay_url = if !sms_config.otp_relay_url.trim().is_empty() {
        sms_config.otp_relay_url.trim().to_string()
    } else {
        "https://mail-api.yuecheng.shop/api/text-relay/eca_tr_DWLd3xXapmgvHPLyOxsCUXOy".to_string()
    };

    pp.log(&format!(
        "📱 [OTP CHALLENGE] Phát hiện Modal OTP! Kết nối SMS Relay: {}",
        relay_url
    ));

    let client = wreq::Client::new();

    let mut last_filled_code = String::new();

    let start_time = std::time::Instant::now();
    let timeout_secs = 120;
    let mut last_log_time = std::time::Instant::now();

    while start_time.elapsed().as_secs() < timeout_secs {
        let elapsed = start_time.elapsed().as_secs();
        if last_log_time.elapsed().as_secs() >= 10 {
            pp.log(&format!(
                "⏳ Đang kiểm tra mã OTP mới trên Relay API... (giây thứ {}/{})",
                elapsed, timeout_secs
            ));
            last_log_time = std::time::Instant::now();
        }

        let still_has_modal = pp
            .eval_bool(
                r#"(() => {
            return !!(
                document.getElementById('ci-ciBasic-0') ||
                document.querySelector('[data-testid="sca-confirm-multi-field"]') ||
                document.querySelector('[id*="ci-"]') ||
                document.querySelector('input[autocomplete="one-time-code"]')
            );
        })()"#,
            )
            .await;

        if !still_has_modal {
            pp.log("ℹ️ Modal OTP đã biến mất trên trình duyệt.");
            *otp_attempts = 0;
            return OtpResult::NoModal;
        }

        if let Ok(resp) = client.get(&relay_url).send().await {
            if let Ok(text) = resp.text().await {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(data_str) = json.get("data").and_then(|d| d.as_str()) {
                        if let Some(code) = extract_6_digit_otp(data_str) {
                            let is_fresh = last_filled_code != code;

                            if is_fresh {
                                pp.log(&format!("🎉 Nhận được mã OTP mới: {}", code));
                                #[allow(unused_assignments)]
                                {
                                    last_filled_code = code.clone();
                                }
                                let digits: Vec<char> = code.chars().collect();
                                for (i, digit) in digits.iter().enumerate().take(6) {
                                    let id = format!("ci-ciBasic-{}", i);
                                    pp.fill(&id, &digit.to_string()).await;
                                    tokio::time::sleep(Duration::from_millis(200)).await;
                                }
                                tokio::time::sleep(Duration::from_secs(3)).await;
                                *otp_attempts = 0;
                                return OtpResult::Filled;
                            }
                        }
                    }
                }
            }
        }

        tokio::time::sleep(Duration::from_secs(4)).await;
    }

    pp.log("⚠️ Quá thời gian 120s chờ OTP tự động. Vui lòng nhập thủ công nếu có!");
    *otp_attempts = 1;
    OtpResult::NoModal
}

pub async fn refill_phone_and_submit(pp: &PaypalPage<'_>) -> Option<String> {
    pp.log("⚠️ Auto mua phone đang tắt. Vui lòng cập nhật hoặc xử lý trên trình duyệt...");
    Some(String::new())
}
