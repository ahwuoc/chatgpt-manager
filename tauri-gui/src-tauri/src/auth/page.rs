use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::time::Duration;

pub(crate) async fn wait_for_element(
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

pub(crate) async fn is_chatgpt_logged_in(page: &chaser_oxide::Page) -> bool {
    page.evaluate(
        r#"(() => {
            return !!(
                document.querySelector('[data-testid="composer-speech-button"]') ||
                document.querySelector('textarea[id="prompt-textarea"]') ||
                document.querySelector('textarea[data-testid="prompt-textarea"]') ||
                document.querySelector('[data-testid="profile-button"]')
            );
        })()"#,
    )
    .await
    .ok()
    .and_then(|value| value.into_value::<bool>().ok())
    .unwrap_or(false)
}

#[derive(Debug, Deserialize)]
pub(crate) struct ResendClickResult {
    pub clicked: bool,
    pub detail: String,
}

pub(crate) async fn click_resend_email(page: &chaser_oxide::Page) -> Result<ResendClickResult> {
    let resend_script = r#"
        (() => {
            const clickableSelector = 'button, a, [role="button"], input[type="button"], input[type="submit"], [tabindex]';
            const looseSelector = `${clickableSelector}, span, div`;
            const needles = [
                'resend',
                'resend email',
                'resend code',
                'send again',
                'send new',
                'new code',
                'another code',
                'try again',
                "didn't receive",
                "didn’t receive",
                'gửi lại',
                'gui lai',
                'mã mới',
                'ma moi'
            ];

            const isVisible = (el) => {
                if (!el) return false;
                const style = window.getComputedStyle(el);
                const rect = el.getBoundingClientRect();
                return style.visibility !== 'hidden' &&
                    style.display !== 'none' &&
                    rect.width > 0 &&
                    rect.height > 0;
            };

            const labelFor = (el) => [
                el?.innerText,
                el?.textContent,
                el?.getAttribute?.('aria-label'),
                el?.getAttribute?.('title'),
                el?.getAttribute?.('name'),
                el?.getAttribute?.('value'),
                el?.getAttribute?.('data-testid')
            ].filter(Boolean).join(' ').replace(/\s+/g, ' ').trim();

            const isDisabled = (el) =>
                !el ||
                el.disabled ||
                el.getAttribute?.('disabled') !== null ||
                el.getAttribute?.('aria-disabled') === 'true' ||
                el.className?.toString().toLowerCase().includes('disabled');

            const activate = (clickable, label) => {
                clickable.scrollIntoView({ block: 'center', inline: 'center' });
                if (window.__moveVirtualMouse) {
                    window.__moveVirtualMouse(clickable);
                }
                if (typeof clickable.focus === 'function') {
                    clickable.focus({ preventScroll: true });
                }

                for (const type of ['pointerdown', 'mousedown', 'mouseup']) {
                    clickable.dispatchEvent(new MouseEvent(type, {
                        bubbles: true,
                        cancelable: true,
                        view: window,
                        buttons: 1
                    }));
                }

                if (typeof clickable.click === 'function') {
                    clickable.click();
                } else {
                    clickable.dispatchEvent(new MouseEvent('click', {
                        bubbles: true,
                        cancelable: true,
                        view: window,
                        buttons: 1
                    }));
                }

                if (
                    clickable.tagName === 'BUTTON' &&
                    (clickable.type || '').toLowerCase() === 'submit' &&
                    clickable.form &&
                    typeof clickable.form.requestSubmit === 'function'
                ) {
                    setTimeout(() => clickable.form.requestSubmit(clickable), 50);
                }

                return JSON.stringify({
                    clicked: true,
                    detail: label.slice(0, 120) || clickable.tagName
                });
            };

            const exact = document.querySelector('button[name="intent"][value="resend"], input[name="intent"][value="resend"]');
            if (exact && isVisible(exact)) {
                const exactLabel = labelFor(exact) || 'intent=resend';
                if (isDisabled(exact)) {
                    return JSON.stringify({
                        clicked: false,
                        detail: `DISABLED_EXACT: ${exactLabel.slice(0, 120)}`
                    });
                }
                return activate(exact, `EXACT: ${exactLabel}`);
            }

            const direct = Array.from(document.querySelectorAll(clickableSelector));
            const loose = Array.from(document.querySelectorAll(looseSelector));
            const candidates = [...new Set([...direct, ...loose])];

            for (const el of candidates) {
                if (!isVisible(el)) continue;
                const label = labelFor(el).toLowerCase();
                if (!needles.some((needle) => label.includes(needle))) continue;

                const clickable = el.matches(clickableSelector)
                    ? el
                    : el.closest(clickableSelector) ||
                        (window.getComputedStyle(el).cursor === 'pointer' ? el : null);

                if (!clickable || !isVisible(clickable)) continue;
                const clickableLabel = labelFor(clickable) || labelFor(el);
                if (isDisabled(clickable)) {
                    return JSON.stringify({
                        clicked: false,
                        detail: `DISABLED: ${clickableLabel.slice(0, 120)}`
                    });
                }

                return activate(clickable, clickableLabel);
            }

            const bodyText = (document.body?.innerText || '').toLowerCase();
            const hasResendText = needles.some((needle) => bodyText.includes(needle));
            return JSON.stringify({
                clicked: false,
                detail: hasResendText ? 'TEXT_FOUND_BUT_NO_CLICKABLE_ELEMENT' : 'NOT_FOUND'
            });
        })()
    "#;

    match page.evaluate(resend_script).await {
        Ok(js_res) => {
            let raw = js_res
                .into_value::<String>()
                .unwrap_or_else(|_| "{\"clicked\":false,\"detail\":\"BAD_JS_VALUE\"}".to_string());
            serde_json::from_str::<ResendClickResult>(&raw)
                .map_err(|e| anyhow!("Lỗi parse kết quả Resend email: {} ({})", e, raw))
        }
        Err(e) => Err(anyhow!("Lỗi bấm Resend email: {:?}", e)),
    }
}

pub(crate) async fn otp_page_has_failure(page: &chaser_oxide::Page) -> bool {
    let script = r#"
        (() => {
            const text = (document.body.innerText || '').toLowerCase();
            return [
                'invalid code',
                'incorrect code',
                'wrong code',
                'expired',
                'code is incorrect',
                'verification failed',
                'mã không đúng',
                'mã đã hết hạn'
            ].some((needle) => text.includes(needle));
        })()
    "#;

    page.evaluate(script)
        .await
        .ok()
        .and_then(|value| value.into_value::<bool>().ok())
        .unwrap_or(false)
}

pub(crate) async fn otp_input_visible(page: &chaser_oxide::Page) -> bool {
    let script = r#"
        (() => {
            const isVisible = (el) => {
                if (!el) return false;
                const style = window.getComputedStyle(el);
                const rect = el.getBoundingClientRect();
                return style.visibility !== 'hidden' &&
                    style.display !== 'none' &&
                    rect.width > 0 &&
                    rect.height > 0;
            };

            const input =
                document.querySelector('input[inputmode="numeric"]') ||
                document.querySelector('input[autocomplete="one-time-code"]') ||
                document.querySelector('input[name*="code" i]') ||
                document.querySelector('input[id*="code" i]') ||
                document.querySelector('input[type="text"]');
            if (!isVisible(input)) return false;

            const text = (document.body.innerText || '').toLowerCase();
            return text.includes('check your inbox') ||
                text.includes('verification code') ||
                text.includes('enter the verification code') ||
                text.includes('code we just sent') ||
                text.includes('resend email') ||
                text.includes('continue with password');
        })()
    "#;

    page.evaluate(script)
        .await
        .ok()
        .and_then(|value| value.into_value::<bool>().ok())
        .unwrap_or(false)
}

pub(crate) async fn login_email_form_visible(page: &chaser_oxide::Page) -> bool {
    let script = r#"
        (() => {
            const isVisible = (el) => {
                if (!el) return false;
                const style = window.getComputedStyle(el);
                const rect = el.getBoundingClientRect();
                return style.visibility !== 'hidden' &&
                    style.display !== 'none' &&
                    rect.width > 0 &&
                    rect.height > 0;
            };

            const input =
                document.querySelector('input[type="email"]') ||
                document.querySelector('input[autocomplete="username"]') ||
                document.querySelector('input[name="email"]');
            if (!isVisible(input)) return false;

            const text = (document.body.innerText || '').toLowerCase();
            const hasLoginCopy =
                text.includes('log in or sign up') ||
                text.includes('email address') ||
                text.includes('continue');
            const hasContinue = Array.from(document.querySelectorAll('button, input[type="submit"], [role="button"]'))
                .some((el) => isVisible(el) && `${el.innerText || ''} ${el.value || ''}`.toLowerCase().includes('continue'));

            return hasLoginCopy || hasContinue;
        })()
    "#;

    page.evaluate(script)
        .await
        .ok()
        .and_then(|value| value.into_value::<bool>().ok())
        .unwrap_or(false)
}

pub(crate) async fn submit_login_email(
    page: &chaser_oxide::Page,
    email: &str,
) -> Result<ResendClickResult> {
    let email_json = serde_json::to_string(email).map_err(|e| anyhow!(e.to_string()))?;
    let script = format!(
        r#"
        (() => {{
            const email = {email_json};
            const isVisible = (el) => {{
                if (!el) return false;
                const style = window.getComputedStyle(el);
                const rect = el.getBoundingClientRect();
                return style.visibility !== 'hidden' &&
                    style.display !== 'none' &&
                    rect.width > 0 &&
                    rect.height > 0;
            }};

            const input =
                document.querySelector('input[type="email"]') ||
                document.querySelector('input[autocomplete="username"]') ||
                document.querySelector('input[name="email"]');
            if (!isVisible(input)) {{
                return JSON.stringify({{ clicked: false, detail: 'EMAIL_INPUT_NOT_FOUND' }});
            }}

            const setValue = (el, value) => {{
                el.focus();
                el.click();
                const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
                if (setter) setter.call(el, value); else el.value = value;
                if (el._valueTracker) el._valueTracker.setValue('');
                el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                el.dispatchEvent(new Event('change', {{ bubbles: true }}));

                const reactKey = Object.keys(el).find((key) => key.startsWith('__reactProps$'));
                if (reactKey && el[reactKey]) {{
                    try {{ el[reactKey].onChange?.({{ target: el, currentTarget: el }}); }} catch (_) {{}}
                    try {{ el[reactKey].onInput?.({{ target: el, currentTarget: el }}); }} catch (_) {{}}
                }}
            }};

            setValue(input, email);
            if (window.__moveVirtualMouse) {{
                window.__moveVirtualMouse(input);
            }}

            const candidates = Array.from(document.querySelectorAll('button, input[type="submit"], [role="button"]'));
            const btn = candidates.find((el) => {{
                if (!isVisible(el)) return false;
                if (el.disabled || el.getAttribute('aria-disabled') === 'true') return false;
                const label = `${{el.innerText || ''}} ${{el.textContent || ''}} ${{el.value || ''}} ${{el.getAttribute('aria-label') || ''}}`.toLowerCase();
                return label.includes('continue') || label.includes('next') || label.includes('tiếp tục');
            }});

            if (btn) {{
                btn.scrollIntoView({{ block: 'center', inline: 'center' }});
                if (window.__moveVirtualMouse) {{
                    window.__moveVirtualMouse(btn);
                }}
                btn.click();
                if (btn.tagName === 'BUTTON' && btn.form && typeof btn.form.requestSubmit === 'function') {{
                    setTimeout(() => btn.form.requestSubmit(btn), 30);
                }}
                return JSON.stringify({{ clicked: true, detail: 'CLICKED_CONTINUE' }});
            }}

            input.dispatchEvent(new KeyboardEvent('keydown', {{ key: 'Enter', code: 'Enter', bubbles: true, cancelable: true }}));
            input.dispatchEvent(new KeyboardEvent('keyup', {{ key: 'Enter', code: 'Enter', bubbles: true, cancelable: true }}));
            if (input.form && typeof input.form.requestSubmit === 'function') {{
                input.form.requestSubmit();
                return JSON.stringify({{ clicked: true, detail: 'REQUEST_SUBMIT_FORM' }});
            }}

            return JSON.stringify({{ clicked: false, detail: 'CONTINUE_BUTTON_NOT_FOUND' }});
        }})()
        "#
    );

    match page.evaluate(script.as_str()).await {
        Ok(js_res) => {
            let raw = js_res
                .into_value::<String>()
                .unwrap_or_else(|_| "{\"clicked\":false,\"detail\":\"BAD_JS_VALUE\"}".to_string());
            serde_json::from_str::<ResendClickResult>(&raw)
                .map_err(|e| anyhow!("Lỗi parse kết quả submit email login: {} ({})", e, raw))
        }
        Err(e) => Err(anyhow!("Lỗi submit email login: {:?}", e)),
    }
}

pub(crate) async fn auth_page_has_unknown_error(page: &chaser_oxide::Page) -> bool {
    let script = r#"
        (() => {
            const text = (document.body.innerText || '').toLowerCase();
            return text.includes('oops, an error occurred') ||
                text.includes('unknown error') ||
                text.includes('something went wrong');
        })()
    "#;

    page.evaluate(script)
        .await
        .ok()
        .and_then(|value| value.into_value::<bool>().ok())
        .unwrap_or(false)
}

pub(crate) async fn click_auth_try_again(page: &chaser_oxide::Page) -> Result<ResendClickResult> {
    let script = r#"
        (() => {
            const candidates = Array.from(document.querySelectorAll('button, a, [role="button"], input[type="button"], input[type="submit"]'));
            const isVisible = (el) => {
                if (!el) return false;
                const style = window.getComputedStyle(el);
                const rect = el.getBoundingClientRect();
                return style.visibility !== 'hidden' &&
                    style.display !== 'none' &&
                    rect.width > 0 &&
                    rect.height > 0;
            };
            const labelFor = (el) => [
                el?.innerText,
                el?.textContent,
                el?.getAttribute?.('aria-label'),
                el?.getAttribute?.('title'),
                el?.getAttribute?.('name'),
                el?.getAttribute?.('value'),
                el?.getAttribute?.('data-dd-action-name')
            ].filter(Boolean).join(' ').replace(/\s+/g, ' ').trim();

            for (const el of candidates) {
                if (!isVisible(el)) continue;
                const label = labelFor(el).toLowerCase();
                if (!label.includes('try again')) continue;

                if (el.disabled || el.getAttribute('aria-disabled') === 'true') {
                    return JSON.stringify({ clicked: false, detail: `DISABLED: ${labelFor(el).slice(0, 120)}` });
                }

                el.scrollIntoView({ block: 'center', inline: 'center' });
                if (window.__moveVirtualMouse) {
                    window.__moveVirtualMouse(el);
                }
                if (typeof el.focus === 'function') {
                    el.focus({ preventScroll: true });
                }
                el.click();
                return JSON.stringify({ clicked: true, detail: labelFor(el).slice(0, 120) || 'Try again' });
            }

            return JSON.stringify({ clicked: false, detail: 'NOT_FOUND' });
        })()
    "#;

    match page.evaluate(script).await {
        Ok(js_res) => {
            let raw = js_res
                .into_value::<String>()
                .unwrap_or_else(|_| "{\"clicked\":false,\"detail\":\"BAD_JS_VALUE\"}".to_string());
            serde_json::from_str::<ResendClickResult>(&raw)
                .map_err(|e| anyhow!("Lỗi parse kết quả Try again: {} ({})", e, raw))
        }
        Err(e) => Err(anyhow!("Lỗi bấm Try again: {:?}", e)),
    }
}

pub(crate) async fn click_signup_one_time_code(
    page: &chaser_oxide::Page,
) -> Result<ResendClickResult> {
    let script = r#"
        (() => {
            const clickableSelector = 'button, a, [role="button"], input[type="button"], input[type="submit"]';
            const isVisible = (el) => {
                const style = window.getComputedStyle(el);
                const rect = el.getBoundingClientRect();
                return style.visibility !== 'hidden' &&
                    style.display !== 'none' &&
                    rect.width > 0 &&
                    rect.height > 0;
            };
            const labelFor = (el) => [
                el.innerText,
                el.textContent,
                el.getAttribute('aria-label'),
                el.getAttribute('title'),
                el.getAttribute('name'),
                el.getAttribute('value'),
                el.getAttribute('data-dd-action-name')
            ].filter(Boolean).join(' ').replace(/\s+/g, ' ').trim();
            const isDisabled = (el) =>
                el.disabled ||
                el.getAttribute('disabled') !== null ||
                el.getAttribute('aria-disabled') === 'true';

            const activate = (el, label) => {
                if (isDisabled(el)) {
                    return JSON.stringify({ clicked: false, detail: `DISABLED: ${label.slice(0, 120)}` });
                }
                el.scrollIntoView({ block: 'center', inline: 'center' });
                if (window.__moveVirtualMouse) {
                    window.__moveVirtualMouse(el);
                }
                if (typeof el.focus === 'function') {
                    el.focus({ preventScroll: true });
                }
                el.click();
                if (
                    el.tagName === 'BUTTON' &&
                    (el.type || '').toLowerCase() === 'submit' &&
                    el.form &&
                    typeof el.form.requestSubmit === 'function'
                ) {
                    setTimeout(() => el.form.requestSubmit(el), 50);
                }
                return JSON.stringify({ clicked: true, detail: label.slice(0, 120) || 'one-time code' });
            };

            const exact = document.querySelector(
                'button[name="intent"][value="passwordless_signup_send_otp"], input[name="intent"][value="passwordless_signup_send_otp"]'
            );
            if (exact && isVisible(exact)) {
                return activate(exact, `EXACT: ${labelFor(exact) || 'passwordless_signup_send_otp'}`);
            }

            const candidates = Array.from(document.querySelectorAll(clickableSelector));
            for (const el of candidates) {
                if (!isVisible(el)) continue;
                const label = labelFor(el).toLowerCase();
                if (
                    label.includes('sign up with a one-time code') ||
                    label.includes('one-time code') ||
                    label.includes('one time code') ||
                    label.includes('passwordless_signup_send_otp')
                ) {
                    return activate(el, labelFor(el));
                }
            }

            return JSON.stringify({ clicked: false, detail: 'NOT_FOUND' });
        })()
    "#;

    match page.evaluate(script).await {
        Ok(js_res) => {
            let raw = js_res
                .into_value::<String>()
                .unwrap_or_else(|_| "{\"clicked\":false,\"detail\":\"BAD_JS_VALUE\"}".to_string());
            serde_json::from_str::<ResendClickResult>(&raw)
                .map_err(|e| anyhow!("Lỗi parse kết quả one-time code: {} ({})", e, raw))
        }
        Err(e) => Err(anyhow!("Lỗi bấm Sign up with one-time code: {:?}", e)),
    }
}

pub(crate) async fn inject_auth_debug_overlay(
    page: &chaser_oxide::Page,
    email: &str,
) -> Result<()> {
    let email_json = serde_json::to_string(email).map_err(|e| anyhow!(e.to_string()))?;
    let script = format!(
        r#"
        (() => {{
            // Set document title
            document.title = '🔥 ' + {email_json} + ' 🔥';

            // Create status overlay
            let status = document.getElementById('auth-bot-overlay');
            if (!status) {{
                status = document.createElement('div');
                status.id = 'auth-bot-overlay';
                status.textContent = '🤖 Đang xử lý: ' + {email_json};
                Object.assign(status.style, {{
                    position: 'fixed',
                    top: '10px',
                    right: '10px',
                    zIndex: '999999999',
                    background: 'rgba(15, 23, 42, 0.9)',
                    color: '#6366f1',
                    border: '1px solid #4f46e5',
                    padding: '8px 12px',
                    fontSize: '14px',
                    fontWeight: 'bold',
                    borderRadius: '6px',
                    pointerEvents: 'none',
                    boxShadow: '0 4px 12px rgba(0,0,0,0.5)',
                    fontFamily: 'system-ui, -apple-system, sans-serif'
                }});
                document.body.appendChild(status);
            }}

            // Inject virtual mouse system
            if (!window.__moveVirtualMouse) {{
                window.__injectVirtualMouse = () => {{
                    let mouse = document.getElementById('virtual-mouse');
                    if (!mouse) {{
                        mouse = document.createElement('div');
                        mouse.id = 'virtual-mouse';
                        Object.assign(mouse.style, {{
                            position: 'fixed',
                            width: '18px',
                            height: '18px',
                            background: 'rgba(239, 68, 68, 0.85)',
                            border: '2px solid #ffffff',
                            borderRadius: '50%',
                            pointerEvents: 'none',
                            zIndex: '9999999999',
                            transition: 'left 0.4s cubic-bezier(0.25, 0.8, 0.25, 1), top 0.4s cubic-bezier(0.25, 0.8, 0.25, 1)',
                            boxShadow: '0 0 12px rgba(239, 68, 68, 0.9)',
                            top: '250px',
                            left: '160px',
                            transform: 'translate(-50%, -50%)'
                        }});
                        document.body.appendChild(mouse);
                    }}
                }};

                window.__moveVirtualMouse = (el) => {{
                    window.__injectVirtualMouse();
                    const mouse = document.getElementById('virtual-mouse');
                    if (!mouse || !el) return;
                    const rect = el.getBoundingClientRect();
                    const x = rect.left + rect.width / 2;
                    const y = rect.top + rect.height / 2;
                    mouse.style.left = x + 'px';
                    mouse.style.top = y + 'px';

                    // Ripple click animation
                    setTimeout(() => {{
                        const ripple = document.createElement('div');
                        Object.assign(ripple.style, {{
                            position: 'fixed',
                            left: x + 'px',
                            top: y + 'px',
                            width: '18px',
                            height: '18px',
                            border: '3px solid rgba(239, 68, 68, 0.85)',
                            borderRadius: '50%',
                            pointerEvents: 'none',
                            zIndex: '9999999998',
                            transform: 'translate(-50%, -50%)',
                            transition: 'width 0.4s ease-out, height 0.4s ease-out, opacity 0.4s ease-out',
                            opacity: '1'
                        }});
                        document.body.appendChild(ripple);
                        setTimeout(() => {{
                            ripple.style.width = '55px';
                            ripple.style.height = '55px';
                            ripple.style.opacity = '0';
                        }}, 15);
                        setTimeout(() => ripple.remove(), 420);
                    }}, 400);
                }};
            }}
            window.__injectVirtualMouse();
            return 'OK';
        }})()
        "#,
        email_json = email_json
    );
    let _ = page.evaluate(script.as_str()).await;
    Ok(())
}

#[allow(dead_code)]
pub(crate) async fn animate_mouse_to_element(
    page: &chaser_oxide::Page,
    selector: &str,
) -> Result<()> {
    let selector_json = serde_json::to_string(selector).map_err(|e| anyhow!(e.to_string()))?;
    let script = format!(
        r#"
        (() => {{
            const el = document.querySelector({selector_json});
            if (el && window.__moveVirtualMouse) {{
                window.__moveVirtualMouse(el);
                return true;
            }}
            return false;
        }})()
        "#,
        selector_json = selector_json
    );
    let _ = page.evaluate(script.as_str()).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    Ok(())
}
