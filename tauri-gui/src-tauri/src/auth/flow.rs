use super::account::Account;
use super::emit_log;
use super::page::{
    auth_page_has_unknown_error, click_auth_try_again, click_resend_email,
    click_signup_one_time_code, is_chatgpt_logged_in, login_email_form_visible, otp_input_visible,
    otp_page_has_failure, submit_login_email, wait_for_element,
};
use super::token::{
    extract_session_access_token, extract_session_access_token_once, save_access_token,
};
use crate::otp::OTPService;
use anyhow::{anyhow, Result};
use chaser_oxide::{Browser, BrowserConfig, ChaserPage};
use chrono::{Datelike, Utc};
use fake::faker::name::en::Name;
use fake::Fake;
use futures::StreamExt;
use std::collections::HashSet;
use std::time::Duration;

const MAX_LOGIN_ATTEMPTS: usize = 60;
const MAX_RESEND_CLICKS: usize = 5;
const MAX_UNKNOWN_AUTH_ERRORS: usize = 20;
const RESEND_COOLDOWN_SECS: i64 = 15;
const OTP_POLL_INTERVAL_MS: u64 = 1_000;

async fn click_resend_with_log(
    app: &tauri::AppHandle,
    page: &chaser_oxide::Page,
    email: &str,
    reason: &str,
) -> Result<bool> {
    emit_log(
        app,
        format!("🔄 [{}] Bấm Resend email ({})...", email, reason),
    );
    match click_resend_email(page).await {
        Ok(result) if result.clicked => {
            emit_log(
                app,
                format!(
                    "✅ [{}] Đã click Resend email: {}. Poll OTP liên tục...",
                    email, result.detail
                ),
            );
            tokio::time::sleep(Duration::from_millis(500)).await;
            Ok(true)
        }
        Ok(result) => {
            emit_log(
                app,
                format!(
                    "⚠️ [{}] Chưa click được Resend email: {}",
                    email, result.detail
                ),
            );
            Ok(false)
        }
        Err(e) => {
            emit_log(app, format!("⚠️ [{}] Lỗi click Resend email: {}", email, e));
            Ok(false)
        }
    }
}

async fn resend_with_guard(
    app: &tauri::AppHandle,
    page: &chaser_oxide::Page,
    email: &str,
    reason: &str,
    resend_clicks: &mut usize,
    otp_not_before_ts: &mut i64,
    last_resend_ts: &mut Option<i64>,
    force: bool,
) -> Result<bool> {
    if *resend_clicks >= MAX_RESEND_CLICKS {
        emit_log(
            app,
            format!(
                "⚠️ [{}] Đã resend {}/{} lần, không bấm thêm để tránh spam.",
                email, resend_clicks, MAX_RESEND_CLICKS
            ),
        );
        return Ok(false);
    }

    let now = Utc::now().timestamp();
    if !force {
        if let Some(last_ts) = *last_resend_ts {
            let elapsed = now - last_ts;
            if elapsed < RESEND_COOLDOWN_SECS {
                emit_log(
                    app,
                    format!(
                        "⏳ [{}] Chưa resend vì còn cooldown {}s ({}).",
                        email,
                        RESEND_COOLDOWN_SECS - elapsed,
                        reason
                    ),
                );
                return Ok(false);
            }
        }
    }

    let resend_started_at = now - 5;
    if click_resend_with_log(app, page, email, reason).await? {
        *otp_not_before_ts = resend_started_at;
        *last_resend_ts = Some(Utc::now().timestamp());
        *resend_clicks += 1;
        return Ok(true);
    }

    Ok(false)
}

async fn recover_unknown_auth_error(
    app: &tauri::AppHandle,
    page: &chaser_oxide::Page,
    email: &str,
) -> Result<bool> {
    if !auth_page_has_unknown_error(page).await {
        return Ok(false);
    }

    emit_log(
        app,
        format!(
            "⚠️ [{}] OpenAI trả về Unknown error sau OTP. Bấm Try again để quay lại form...",
            email
        ),
    );

    match click_auth_try_again(page).await {
        Ok(result) if result.clicked => {
            emit_log(
                app,
                format!(
                    "✅ [{}] Đã click Try again: {}. Chờ UI cập nhật 1s...",
                    email, result.detail
                ),
            );
            tokio::time::sleep(Duration::from_secs(1)).await;
            Ok(true)
        }
        Ok(result) => {
            emit_log(
                app,
                format!(
                    "⚠️ [{}] Không tìm thấy nút Try again: {}",
                    email, result.detail
                ),
            );
            Ok(false)
        }
        Err(e) => {
            emit_log(app, format!("⚠️ [{}] Lỗi click Try again: {}", email, e));
            Err(e)
        }
    }
}

async fn save_existing_session_token(
    app: &tauri::AppHandle,
    page: &chaser_oxide::Page,
    email: &str,
    context: &str,
    retry: bool,
) -> bool {
    let session = if retry {
        extract_session_access_token(page).await
    } else {
        extract_session_access_token_once(page).await
    };

    if let Some(session) = session {
        if !session.email.eq_ignore_ascii_case(email) {
            emit_log(
                app,
                format!(
                    "⚠️ [{}] Profile đang đăng nhập email khác ({}) nên không lưu token cho account này.",
                    email, session.email
                ),
            );
            return false;
        }

        emit_log(
            app,
            format!(
                "✅ [{}] {}. Lấy Access Token ngay, bỏ qua bước login.",
                email, context
            ),
        );
        save_access_token(email, &session.token);
        emit_log(
            app,
            format!("🔑 [{}] Trích xuất Access Token thành công!", email),
        );
        return true;
    }

    false
}

async fn maybe_switch_signup_password_to_otp(
    app: &tauri::AppHandle,
    chaser: &ChaserPage,
    page: &chaser_oxide::Page,
    email: &str,
    otp_not_before_ts: &mut i64,
    last_resend_ts: &mut Option<i64>,
) -> Result<bool> {
    let current_url = chaser.url().await.unwrap_or(None).unwrap_or_default();
    let on_password_page = current_url.contains("/create-account/password");

    if !on_password_page {
        let has_button = page
            .evaluate(
                r#"(() => !!document.querySelector(
                    'button[name="intent"][value="passwordless_signup_send_otp"], input[name="intent"][value="passwordless_signup_send_otp"]'
                ))()"#,
            )
            .await
            .ok()
            .and_then(|res| res.into_value::<bool>().ok())
            .unwrap_or(false);
        if !has_button {
            return Ok(false);
        }
    }

    emit_log(
        app,
        format!(
            "🔁 [{}] Đang ở trang tạo password, chuyển lại đăng ký bằng OTP...",
            email
        ),
    );

    match click_signup_one_time_code(page).await {
        Ok(result) if result.clicked => {
            let started_at = Utc::now().timestamp() - 5;
            *otp_not_before_ts = started_at;
            *last_resend_ts = Some(Utc::now().timestamp());
            emit_log(
                app,
                format!(
                    "✅ [{}] Đã click one-time code: {}. Bắt đầu poll OTP mới...",
                    email, result.detail
                ),
            );
            tokio::time::sleep(Duration::from_millis(800)).await;
            Ok(true)
        }
        Ok(result) => {
            emit_log(
                app,
                format!(
                    "⚠️ [{}] Chưa click được one-time code: {}",
                    email, result.detail
                ),
            );
            Ok(false)
        }
        Err(e) => {
            emit_log(
                app,
                format!("⚠️ [{}] Lỗi click one-time code: {}", email, e),
            );
            Err(e)
        }
    }
}

async fn maybe_complete_about_you(
    app: &tauri::AppHandle,
    chaser: &ChaserPage,
    email: &str,
) -> Result<bool> {
    let page = chaser.raw_page();
    let detected = page
        .evaluate(
            r#"(() => {
                const url = (window.location.pathname || '').toLowerCase();
                const text = (document.body?.innerText || '').toLowerCase();
                const hasNameInput = !!document.querySelector('input[name="name"], input[autocomplete="name"]');
                const hasAgeInput = !!document.querySelector('input[name="age"]');
                const hasBirthdayInput =
                    !!document.querySelector('input[name="birthday"]') ||
                    !!document.querySelector('[data-type="month"]');
                return (
                    url.includes('/about-you') ||
                    text.includes('how old are you') ||
                    text.includes("let's confirm your age") ||
                    text.includes('confirm your age')
                ) && hasNameInput && (hasAgeInput || hasBirthdayInput);
            })()"#,
        )
        .await
        .ok()
        .and_then(|res| res.into_value::<bool>().ok())
        .unwrap_or(false);

    if !detected {
        return Ok(false);
    }

    let fake_name: String = Name().fake();
    let fake_age: i32 = rand::random_range(22..45);
    let now_year = Utc::now().year();
    let year = now_year - fake_age;
    let month: u32 = rand::random_range(1..=12);
    let day: u32 = rand::random_range(1..=28);
    let birthday_iso = format!("{year:04}-{month:02}-{day:02}");

    let name_json = serde_json::to_string(&fake_name).map_err(|e| anyhow!(e.to_string()))?;
    let age_json = serde_json::to_string(&fake_age.to_string()).map_err(|e| anyhow!(e.to_string()))?;
    let birthday_json = serde_json::to_string(&birthday_iso).map_err(|e| anyhow!(e.to_string()))?;

    emit_log(
        app,
        format!(
            "📝 [{}] Phát hiện form confirm age. Điền tự động (Tên: {}, Age: {}, DOB: {})...",
            email, fake_name, fake_age, birthday_iso
        ),
    );

    let script_template = r#"
        (() => {
            const fullName = __FULL_NAME__;
            const ageValue = __AGE__;
            const birthdayIso = __BIRTHDAY__;
            const [year, month, day] = birthdayIso.split('-');

            const setInputValue = (el, value) => {
                if (!el) return false;
                const nextValue = String(value ?? '');
                el.focus();
                const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
                if (setter) setter.call(el, nextValue); else el.value = nextValue;
                if (el._valueTracker) el._valueTracker.setValue('');
                el.dispatchEvent(new Event('input', { bubbles: true }));
                el.dispatchEvent(new Event('change', { bubbles: true }));
                const reactKey = Object.keys(el).find((key) => key.startsWith('__reactProps$'));
                if (reactKey && el[reactKey]) {
                    try { el[reactKey].onInput?.({ target: el, currentTarget: el }); } catch (_) {}
                    try { el[reactKey].onChange?.({ target: el, currentTarget: el }); } catch (_) {}
                }
                return true;
            };

            const setSegmentValue = (selector, value) => {
                const el = document.querySelector(selector);
                if (!el) return false;
                el.focus();
                if (el.isContentEditable) {
                    el.textContent = value;
                    el.dispatchEvent(new InputEvent('input', {
                        bubbles: true,
                        inputType: 'insertText',
                        data: value
                    }));
                    el.dispatchEvent(new Event('change', { bubbles: true }));
                    el.dispatchEvent(new Event('blur', { bubbles: true }));
                } else {
                    setInputValue(el, value);
                }
                return true;
            };

            const nameInput =
                document.querySelector('input[name="name"]') ||
                document.querySelector('input[autocomplete="name"]');
            if (!nameInput) return 'NAME_INPUT_NOT_FOUND';
            setInputValue(nameInput, fullName);

            const ageInput =
                document.querySelector('input[name="age"]') ||
                document.querySelector('input[type="number"][name*="age" i]');
            const ageOk = setInputValue(ageInput, ageValue);

            const monthOk = setSegmentValue('[data-type="month"]', month);
            const dayOk = setSegmentValue('[data-type="day"]', day);
            const yearOk = setSegmentValue('[data-type="year"]', year);

            const hiddenBirthday = document.querySelector('input[name="birthday"]');
            const hiddenBirthdayOk = setInputValue(hiddenBirthday, birthdayIso);

            if (!ageOk && !monthOk && !dayOk && !yearOk) {
                return 'AGE_OR_BIRTHDAY_INPUT_NOT_FOUND';
            }

            const submitBtn =
                document.querySelector('button[type="submit"]') ||
                Array.from(document.querySelectorAll('button, [role="button"]'))
                    .find((btn) => {
                        const label = `${btn.textContent || ''} ${btn.getAttribute('aria-label') || ''}`.toLowerCase();
                        return label.includes('finish creating account') || label.includes('finish');
                    });
            if (!submitBtn) return 'SUBMIT_BUTTON_NOT_FOUND';
            if (submitBtn.disabled || submitBtn.getAttribute('aria-disabled') === 'true') {
                return 'SUBMIT_BUTTON_DISABLED';
            }

            submitBtn.scrollIntoView({ block: 'center', inline: 'center' });
            submitBtn.click();
            if (submitBtn.form && typeof submitBtn.form.requestSubmit === 'function') {
                setTimeout(() => submitBtn.form.requestSubmit(submitBtn), 50);
            }

            return `SUBMITTED age=${ageOk} m=${monthOk} d=${dayOk} y=${yearOk} hiddenBirthday=${hiddenBirthdayOk}`;
        })()
    "#;

    let fill_script = script_template
        .replace("__FULL_NAME__", &name_json)
        .replace("__AGE__", &age_json)
        .replace("__BIRTHDAY__", &birthday_json);

    let fill_result = page
        .evaluate(fill_script.as_str())
        .await
        .ok()
        .and_then(|res| res.into_value::<String>().ok())
        .unwrap_or_else(|| "EVAL_FAILED".to_string());

    emit_log(
        app,
        format!("🧾 [{}] Kết quả submit confirm age: {}", email, fill_result),
    );
    tokio::time::sleep(Duration::from_secs(4)).await;
    Ok(true)
}

async fn page_is_rate_limited(page: &chaser_oxide::Page) -> bool {
    page.evaluate("document.body.innerText")
        .await
        .ok()
        .and_then(|res| res.into_value::<String>().ok())
        .map(|text| {
            text.contains("Too many attempts")
                || text.contains("Too many tries")
                || text.contains("max_check_attempts")
        })
        .unwrap_or(false)
}

async fn fill_otp(page: &chaser_oxide::Page, otp: &str) -> Result<bool> {
    let otp_input = match page
        .find_element("input[type='text'], input[inputmode='numeric']")
        .await
    {
        Ok(input) => input,
        Err(_) => return Ok(false),
    };

    let _ = page
        .evaluate(
            r#"
            (() => {
                const el = document.querySelector("input[type='text'], input[inputmode='numeric']");
                if (el) {
                    el.value = '';
                    el.dispatchEvent(new Event('input', { bubbles: true }));
                }
            })()
        "#,
        )
        .await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    otp_input.click().await?;
    tokio::time::sleep(Duration::from_millis(300)).await;

    for c in otp.chars() {
        otp_input.type_str(&c.to_string()).await?;
        tokio::time::sleep(Duration::from_millis(120)).await;
    }

    tokio::time::sleep(Duration::from_millis(400)).await;
    otp_input.press_key("Enter").await?;
    Ok(true)
}

pub(crate) async fn run_login(
    app: tauri::AppHandle,
    account: Account,
    slot_index: usize,
    max_cols: usize,
) -> Result<()> {
    let start = std::time::Instant::now();
    let email = account.email.clone();

    emit_log(
        &app,
        format!(
            "🌐 [{}] Khởi động trình duyệt chaser-oxide (Stealth, Slot: {})...",
            email, slot_index
        ),
    );

    let width: u32 = 320;
    let height: u32 = 540;

    let (x, y) = crate::utils::tiled_window_position(slot_index, max_cols, width, height);
    let window_pos_arg = format!("--window-position={},{}", x, y);
    let window_class = crate::utils::browser_window_class("", &email, slot_index);
    let user_data_dir = crate::utils::browser_profile_dir(&app, "", &email)?;

    emit_log(
        &app,
        format!("🗂️ [{}] Profile dir: {}", email, user_data_dir.display()),
    );

    let builder = BrowserConfig::builder()
        .with_head()
        .window_size(width, height)
        .arg(window_pos_arg)
        .arg(format!("--class={}", window_class))
        .arg("--ozone-platform=x11")
        .arg("--no-first-run")
        .arg("--hide-crash-restore-bubble")
        .arg("--disable-features=InfiniteSessionRestore")
        .arg("--test-type")
        .user_data_dir(user_data_dir.to_string_lossy().into_owned());
    let config = crate::us_browser_proxy::apply_to_browser_builder(builder, &app, "AUTH", &email)?
        .build()
        .map_err(|e| anyhow!("Lỗi cấu hình chaser-oxide: {}", e))?;

    let (browser, mut handler) = Browser::launch(config).await?;
    tokio::spawn(async move { while let Some(_) = handler.next().await {} });
    let page = browser.new_page("about:blank").await?;
    crate::us_browser_proxy::authenticate_page(&page, &app, "AUTH", &email).await?;
    let _ = page
        .evaluate(format!("document.title = '{}';", window_class))
        .await;
    crate::utils::force_tile_window(&window_class, x, y, width, height).await;

    let chaser = ChaserPage::new(page);
    crate::utils::apply_fingerprint_profile(&chaser, &app, "AUTH", &email).await?;
    let raw = chaser.raw_page();
    let _ = super::page::inject_auth_debug_overlay(raw, &email).await;

    chaser.goto("https://chatgpt.com/").await?;
    if save_existing_session_token(&app, raw, &email, "Profile đã có session ChatGPT", false).await
    {
        return Ok(());
    }

    for _ in 0..6 {
        if save_existing_session_token(&app, raw, &email, "Profile đã có session ChatGPT", false)
            .await
        {
            return Ok(());
        }
        if is_chatgpt_logged_in(raw).await {
            emit_log(
                &app,
                format!(
                    "✅ [{}] Profile có giao diện ChatGPT. Xác thực session/token...",
                    email
                ),
            );
            if save_existing_session_token(&app, raw, &email, "Session ChatGPT hợp lệ", true).await
            {
                return Ok(());
            } else {
                emit_log(
                    &app,
                    format!(
                        "⚠️ [{}] Có giao diện ChatGPT nhưng session/token chưa hợp lệ cho email này, chuyển sang login.",
                        email
                    ),
                );
                break;
            }
        }
        if login_email_form_visible(raw).await {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    chaser.goto("https://chatgpt.com/auth/login").await?;
    if save_existing_session_token(&app, raw, &email, "Profile đã có session ChatGPT", false).await
    {
        return Ok(());
    }

    if is_chatgpt_logged_in(raw).await {
        emit_log(
            &app,
            format!(
                "✅ [{}] Profile đã đăng nhập sẵn. Lấy Access Token ngay...",
                email
            ),
        );
        if save_existing_session_token(&app, raw, &email, "Profile đã đăng nhập sẵn", true).await
        {
            return Ok(());
        } else {
            emit_log(
                &app,
                format!(
                    "⚠️ [{}] Giao diện đã vào ChatGPT nhưng token không khớp email account, tiếp tục luồng login.",
                    email
                ),
            );
        }
        return Ok(());
    }

    emit_log(&app, format!("⏳ [{}] Chờ trang đăng nhập...", email));

    let email_input = wait_for_element(
        raw,
        "input[type='email'], input[autocomplete='username']",
        30,
    )
    .await?;

    let otp_service = OTPService::new();
    let mut rejected_otps: HashSet<String> = HashSet::new();
    let mut submitted_otps: HashSet<String> = HashSet::new();
    let mut last_submitted_otp: Option<String> = None;
    emit_log(
        &app,
        format!(
            "🧭 [{}] Quét baseline OTP cũ trước khi submit email...",
            email
        ),
    );
    match otp_service
        .fetch_latest_otp_after(
            &account.email,
            &account.password,
            account.session_token.as_deref(),
            account.account_id.as_deref(),
            None,
        )
        .await
    {
        Ok(Some(old_otp)) => {
            emit_log(
                &app,
                format!(
                    "🧭 [{}] Thấy OTP hiện có: {}. Nếu trang yêu cầu code, sẽ dùng mã mới nhất trong inbox.",
                    email, old_otp
                ),
            );
        }
        Ok(None) => {
            emit_log(
                &app,
                format!("🧭 [{}] Không thấy OTP cũ, bắt đầu sạch.", email),
            );
        }
        Err(e) => {
            emit_log(
                &app,
                format!("⚠️ [{}] Không quét được baseline OTP cũ: {:?}", email, e),
            );
        }
    }

    email_input.click().await?;
    tokio::time::sleep(Duration::from_millis(300)).await;

    emit_log(&app, format!("📧 [{}] Nhập email...", email));
    for c in email.chars() {
        email_input.type_str(&c.to_string()).await?;
        tokio::time::sleep(Duration::from_millis(80)).await;
    }

    tokio::time::sleep(Duration::from_millis(400)).await;
    let mut otp_not_before_ts = Utc::now().timestamp() - 5;
    email_input.press_key("Enter").await?;
    emit_log(
        &app,
        format!(
            "🕒 [{}] Đặt mốc OTP sau khi submit email: {}",
            email, otp_not_before_ts
        ),
    );
    tokio::time::sleep(Duration::from_millis(800)).await;

    let mut logged_in = false;
    let mut resend_clicks = 0usize;
    let mut unknown_auth_errors = 0usize;
    let mut last_resend_ts = Some(otp_not_before_ts);
    let mut last_email_submit_ts = Some(Utc::now().timestamp());

    for attempt in 1..=MAX_LOGIN_ATTEMPTS {
        let _ = super::page::inject_auth_debug_overlay(raw, &email).await;
        if recover_unknown_auth_error(&app, raw, &email).await? {
            unknown_auth_errors += 1;
            if unknown_auth_errors >= MAX_UNKNOWN_AUTH_ERRORS {
                emit_log(
                    &app,
                    format!(
                        "❌ [{}] OpenAI Unknown error {} lần liên tiếp, dừng account này để tránh lặp Try again/Resend.",
                        email, unknown_auth_errors
                    ),
                );
                break;
            }
            if let Some(otp) = last_submitted_otp.take() {
                rejected_otps.insert(otp.clone());
                submitted_otps.remove(&otp);
                emit_log(
                    &app,
                    format!(
                        "♻️ [{}] Mã {} đã dẫn tới Unknown error, sẽ chờ/resend mã khác.",
                        email, otp
                    ),
                );
            }
            resend_with_guard(
                &app,
                raw,
                &email,
                "sau Unknown error",
                &mut resend_clicks,
                &mut otp_not_before_ts,
                &mut last_resend_ts,
                true,
            )
            .await?;
            continue;
        }

        if page_is_rate_limited(raw).await {
            emit_log(
                &app,
                format!(
                    "❌ [{}] Đăng nhập thất bại: bị OpenAI chặn do thử quá nhiều lần.",
                    email
                ),
            );
            break;
        }

        if maybe_complete_about_you(&app, &chaser, &email).await? {
            continue;
        }

        if maybe_switch_signup_password_to_otp(
            &app,
            &chaser,
            raw,
            &email,
            &mut otp_not_before_ts,
            &mut last_resend_ts,
        )
        .await?
        {
            continue;
        }

        let current_url = chaser.url().await.unwrap_or(None).unwrap_or_default();
        if current_url.contains("/auth/login") && login_email_form_visible(raw).await {
            let now = Utc::now().timestamp();
            let can_submit_email = last_email_submit_ts
                .map(|last_ts| now - last_ts >= 8)
                .unwrap_or(true);

            if can_submit_email {
                emit_log(
                    &app,
                    format!(
                        "🔁 [{}] Phát hiện quay lại /auth/login. Điền lại email theo trạng thái trang...",
                        email
                    ),
                );
                match submit_login_email(raw, &email).await {
                    Ok(result) if result.clicked => {
                        otp_not_before_ts = Utc::now().timestamp() - 5;
                        last_email_submit_ts = Some(Utc::now().timestamp());
                        last_resend_ts = Some(otp_not_before_ts);
                        submitted_otps.clear();
                        last_submitted_otp = None;
                        emit_log(
                            &app,
                            format!(
                                "✅ [{}] Đã submit lại email: {}. Mốc OTP mới: {}",
                                email, result.detail, otp_not_before_ts
                            ),
                        );
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                    Ok(result) => {
                        emit_log(
                            &app,
                            format!(
                                "⚠️ [{}] Chưa submit lại được email login: {}",
                                email, result.detail
                            ),
                        );
                        last_email_submit_ts = Some(now);
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                    Err(e) => {
                        emit_log(
                            &app,
                            format!("⚠️ [{}] Lỗi submit lại email login: {}", email, e),
                        );
                        last_email_submit_ts = Some(now);
                    }
                }
            } else if attempt == 1 || attempt % 5 == 0 {
                emit_log(
                    &app,
                    format!(
                        "⏳ [{}] Đang ở form email login, chờ trang xử lý trước khi submit lại...",
                        email
                    ),
                );
            }

            tokio::time::sleep(Duration::from_millis(OTP_POLL_INTERVAL_MS)).await;
            continue;
        }

        if current_url.contains("chatgpt.com") && !current_url.contains("auth") {
            logged_in = true;
            break;
        }

        let on_otp_page =
            current_url.contains("email-verification") || otp_input_visible(raw).await;
        emit_log(
            &app,
            format!("🔑 [{}] Đang quét OTP lần {}...", email, attempt),
        );
        let otp_cutoff = Some(otp_not_before_ts);
        let otp_res = otp_service
            .fetch_latest_otp_after(
                &account.email,
                &account.password,
                account.session_token.as_deref(),
                account.account_id.as_deref(),
                otp_cutoff,
            )
            .await;

        match otp_res {
            Ok(Some(otp)) if rejected_otps.contains(&otp) => {
                if attempt == 1 || attempt % 5 == 0 {
                    emit_log(
                        &app,
                        format!(
                            "♻️ [{}] Vẫn thấy OTP đã bị từ chối {}, tiếp tục chờ mã khác...",
                            email, otp
                        ),
                    );
                }

                let can_resend = last_resend_ts
                    .map(|last_ts| Utc::now().timestamp() - last_ts >= RESEND_COOLDOWN_SECS)
                    .unwrap_or(true);
                if can_resend {
                    resend_with_guard(
                        &app,
                        raw,
                        &email,
                        "OTP đã bị từ chối lặp lại",
                        &mut resend_clicks,
                        &mut otp_not_before_ts,
                        &mut last_resend_ts,
                        false,
                    )
                    .await?;
                }
            }
            Ok(Some(otp)) if submitted_otps.contains(&otp) => {
                if attempt == 1 || attempt % 5 == 0 {
                    emit_log(
                        &app,
                        format!(
                            "⏳ [{}] Mã {} đã nhập rồi, đang chờ OpenAI xử lý...",
                            email, otp
                        ),
                    );
                }
            }
            Ok(Some(otp)) => {
                unknown_auth_errors = 0;
                let otp_context = if on_otp_page {
                    "OTP mới nhất trên trang xác minh"
                } else {
                    "OTP mới"
                };
                emit_log(
                    &app,
                    format!("✅ [{}] Tìm thấy {}: {}", email, otp_context, otp),
                );

                let check_url = chaser.url().await.unwrap_or(None).unwrap_or_default();
                if check_url.contains("/about-you") {
                    continue;
                }

                if !fill_otp(raw, &otp).await? {
                    emit_log(
                        &app,
                        format!("⚠️ [{}] Chưa tìm thấy ô nhập OTP trên trang.", email),
                    );
                    continue;
                }
                submitted_otps.insert(otp.clone());
                last_submitted_otp = Some(otp.clone());

                tokio::time::sleep(Duration::from_secs(6)).await;

                if recover_unknown_auth_error(&app, raw, &email).await? {
                    unknown_auth_errors += 1;
                    rejected_otps.insert(otp.clone());
                    submitted_otps.remove(&otp);
                    last_submitted_otp = None;
                    if unknown_auth_errors >= MAX_UNKNOWN_AUTH_ERRORS {
                        emit_log(
                            &app,
                            format!(
                                "❌ [{}] OpenAI Unknown error {} lần liên tiếp sau OTP, dừng account này để tránh lặp.",
                                email, unknown_auth_errors
                            ),
                        );
                        break;
                    }
                    if resend_clicks >= MAX_RESEND_CLICKS {
                        emit_log(
                            &app,
                            format!(
                                "❌ [{}] Đã resend {} lần, dừng account này để tránh spam.",
                                email, resend_clicks
                            ),
                        );
                        break;
                    }
                    resend_with_guard(
                        &app,
                        raw,
                        &email,
                        "OpenAI Unknown error",
                        &mut resend_clicks,
                        &mut otp_not_before_ts,
                        &mut last_resend_ts,
                        true,
                    )
                    .await?;
                    continue;
                }

                if otp_page_has_failure(raw).await {
                    emit_log(&app, format!("⚠️ [{}] OTP bị từ chối hoặc hết hạn.", email));
                    rejected_otps.insert(otp.clone());
                    submitted_otps.remove(&otp);
                    last_submitted_otp = None;
                    if resend_clicks >= MAX_RESEND_CLICKS {
                        emit_log(
                            &app,
                            format!(
                                "❌ [{}] Đã resend {} lần, dừng account này để tránh spam.",
                                email, resend_clicks
                            ),
                        );
                        break;
                    }
                    resend_with_guard(
                        &app,
                        raw,
                        &email,
                        "OTP fail/hết hạn",
                        &mut resend_clicks,
                        &mut otp_not_before_ts,
                        &mut last_resend_ts,
                        true,
                    )
                    .await?;
                }
            }
            Ok(None) => {
                if attempt == 1 || attempt % 5 == 0 {
                    emit_log(
                        &app,
                        format!(
                            "⏳ [{}] Chưa thấy OTP mới, tiếp tục poll realtime...",
                            email
                        ),
                    );
                }

                let can_resend = last_resend_ts
                    .map(|last_ts| Utc::now().timestamp() - last_ts >= RESEND_COOLDOWN_SECS)
                    .unwrap_or(true);
                if can_resend {
                    resend_with_guard(
                        &app,
                        raw,
                        &email,
                        "chưa thấy OTP mới trên Microsoft Graph",
                        &mut resend_clicks,
                        &mut otp_not_before_ts,
                        &mut last_resend_ts,
                        false,
                    )
                    .await?;
                }
            }
            Err(e) => {
                emit_log(
                    &app,
                    format!(
                        "❌ [{}] Lỗi kết nối API OTP hoặc parse mail: {:?}",
                        email, e
                    ),
                );
            }
        }

        tokio::time::sleep(Duration::from_millis(OTP_POLL_INTERVAL_MS)).await;
    }

    if logged_in {
        emit_log(
            &app,
            format!(
                "🎉 [{}] ĐĂNG NHẬP THÀNH CÔNG trong {:.1}s!",
                email,
                start.elapsed().as_secs_f64()
            ),
        );

        if !save_existing_session_token(&app, raw, &email, "Đăng nhập OK", true).await {
            emit_log(
                &app,
                format!(
                    "⚠️ [{}] Đăng nhập OK nhưng session/token chưa hợp lệ hoặc không khớp email.",
                    email
                ),
            );
        }
    } else {
        emit_log(&app, format!("❌ [{}] Đăng nhập thất bại.", email));
    }

    Ok(())
}
