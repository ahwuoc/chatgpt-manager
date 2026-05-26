use chaser_oxide::Page;
use std::time::Duration;
use tauri::Emitter;

use crate::js_helpers::{JS_FIND_EL, JS_IS_VISIBLE};

pub fn js_fill(selector: &str, val: &str) -> String {
    format!(
        r#"(() => {{
            {JS_FIND_EL}
            let el = findEl(document, {sel:?});
            if (!el) return 'NOT_FOUND:' + {sel:?};
            el.focus();
            el.click();

            const proto = el.tagName === 'SELECT' ? HTMLSelectElement.prototype
                        : el.tagName === 'TEXTAREA' ? HTMLTextAreaElement.prototype
                        : HTMLInputElement.prototype;
            const ns = Object.getOwnPropertyDescriptor(proto, 'value');
            if (ns && ns.set) ns.set.call(el, {val:?}); else el.value = {val:?};

            const tracker = el._valueTracker;
            if (tracker) tracker.setValue('');

            el.dispatchEvent(new Event('input', {{ bubbles: true }}));
            el.dispatchEvent(new Event('change', {{ bubbles: true }}));

            const rk = Object.keys(el).find(k => k.startsWith('__reactProps$'));
            if (rk && el[rk]) {{
                if (el[rk].onChange) el[rk].onChange({{ target: el, currentTarget: el }});
                if (el[rk].onInput) el[rk].onInput({{ target: el, currentTarget: el }});
            }}

            const fk = Object.keys(el).find(k => k.startsWith('__reactFiber$'));
            if (fk) {{
                let fiber = el[fk];
                for (let i = 0; i < 20 && fiber; i++) {{
                    const p = fiber.memoizedProps || fiber.pendingProps;
                    if (p && p.onChange) {{
                        try {{ p.onChange({{ target: el, currentTarget: el }}); }} catch(e) {{}}
                        break;
                    }}
                    fiber = fiber.return;
                }}
            }}

            el.dispatchEvent(new Event('blur', {{ bubbles: true }}));
            return 'OK:' + el.value;
        }})()"#,
        JS_FIND_EL = JS_FIND_EL,
        sel = selector,
        val = val,
    )
}

pub fn js_click_btn(selector: &str) -> String {
    format!(
        r#"(() => {{
            {JS_FIND_EL}
            let btn = findEl(document, {sel:?});
            if (btn && !btn.disabled) {{ btn.click(); return 'CLICKED'; }}
            return 'NOT_FOUND';
        }})()"#,
        JS_FIND_EL = JS_FIND_EL,
        sel = selector,
    )
}

pub struct PaypalPage<'a> {
    pub page: &'a Page,
    pub email: &'a str,
    pub app: tauri::AppHandle,
}

impl<'a> PaypalPage<'a> {
    pub fn new(page: &'a Page, email: &'a str, app: tauri::AppHandle) -> Self {
        Self { page, email, app }
    }

    pub async fn eval_str(&self, script: &str) -> String {
        match self.page.evaluate(script).await {
            Ok(v) => match v.into_value::<String>() {
                Ok(s) => s,
                Err(e) => {
                    let err_str = e.to_string();
                    if !err_str.contains("No value found") {
                        self.log(&format!("❌ [eval_str] Lỗi chuyển đổi kết quả: {}", e));
                    }
                    String::new()
                }
            },
            Err(e) => {
                self.log(&format!("❌ [eval_str] Lỗi thực thi JS: {:?}", e));
                String::new()
            }
        }
    }

    pub async fn eval_bool(&self, script: &str) -> bool {
        match self.page.evaluate(script).await {
            Ok(v) => match v.into_value::<bool>() {
                Ok(b) => b,
                Err(e) => {
                    self.log(&format!("❌ [eval_bool] Lỗi chuyển đổi kết quả: {}", e));
                    false
                }
            },
            Err(e) => {
                self.log(&format!("❌ [eval_bool] Lỗi thực thi JS: {:?}", e));
                false
            }
        }
    }

    pub async fn fill(&self, selector: &str, val: &str) {
        let res = self.eval_str(&js_fill(selector, val)).await;
        self.log(&format!(
            "      [DEBUG fill] selector: {}, result: {}",
            selector, res
        ));
    }

    pub async fn click(&self, selector: &str) -> bool {
        self.eval_str(&js_click_btn(selector)).await == "CLICKED"
    }

    pub async fn wait_for_id(&self, id: &str, timeout_secs: u64) -> bool {
        let script = format!(
            "(() => {{\n{}\nreturn !!findEl(document, {:?});\n}})()",
            JS_FIND_EL,
            id
        );
        let ticks = timeout_secs * 2;
        for _ in 0..ticks {
            if self.eval_bool(&script).await {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        false
    }

    pub async fn has_email_input(&self) -> bool {
        let script = format!(
            "(() => {{\n{}\n{}\n}})()",
            JS_IS_VISIBLE,
            r#"
            const el = document.getElementById('login_email') ||
                       document.getElementById('email') ||
                       document.getElementById('onboardingFlowEmail') ||
                       document.querySelector('input[type="email"], input[name="login_email"], input[name="email"]');
            return isVisible(el);
            "#
        );
        self.eval_bool(&script).await
    }

    pub async fn is_initial_create_account_prompt(&self) -> bool {
        let script = format!(
            "(() => {{\n{}\n{}\n}})()",
            JS_IS_VISIBLE,
            r#"
            const submitForm = document.getElementById('publicCredentialSubmitForm');
            const createForm = document.querySelector('form[data-testid="create-account-form"]');
            const createBtn = createForm?.querySelector(
                "button[data-atomic-wait-task='login_create_account'], button[type='submit']"
            );
            const createAccountEmailForm = document.querySelector('form[data-testid="emailForm"]');
            
            const emailEl = document.getElementById('login_email') ||
                            document.getElementById('email') ||
                            document.querySelector('input[type="email"]');
            const hasEmail = isVisible(emailEl);
            
            return !!submitForm && !!createForm && !!createBtn && !createAccountEmailForm && !hasEmail;
            "#
        );
        self.eval_bool(&script).await
    }

    pub async fn is_create_account_email_form(&self) -> bool {
        self.eval_bool(r#"(() => {
            const form = document.querySelector('form[data-testid="emailForm"]');
            const emailInput =
                document.getElementById('login_email') ||
                document.querySelector('form[data-testid="emailForm"] input[name="login_email"]');
            const continueBtn = document.querySelector(
                "button[data-atomic-wait-intent='Continue_To_Payment'], button[data-testid='continueButton']"
            );
            return !!form && !!emailInput && !!continueBtn;
        })()"#).await
    }

    pub async fn is_waiting_for_otp_challenge(&self) -> bool {
        let script = format!(
            "(() => {{\n{}\n{}\n}})()",
            JS_IS_VISIBLE,
            r#"
            const text = (document.body?.innerText || '').toLowerCase();
            const looksLikeCode =
                text.includes('one-time code') ||
                text.includes('one time code') ||
                text.includes('security code') ||
                text.includes('verification code') ||
                text.includes('enter the code') ||
                text.includes('confirm your phone') ||
                text.includes('sent you a code') ||
                text.includes('sent a code') ||
                text.includes('mã xác minh') ||
                text.includes('mã bảo mật');

            const directOtpNode =
                document.getElementById('ci-ciBasic-0') ||
                document.querySelector('[data-testid="sca-confirm-multi-field"]') ||
                document.querySelector('input[autocomplete="one-time-code"]') ||
                document.querySelector('input[name*="otp" i], input[id*="otp" i]');
            if (isVisible(directOtpNode)) return true;

            const exceedNode =
                document.querySelector('[data-testid="exceed-main"]') ||
                document.querySelector('[data-testid="primary-button-exceed"]');
            if (isVisible(exceedNode)) return true;

            const canResend = Array.from(document.querySelectorAll('button, a, [role="button"]')).some((el) => {
                if (!isVisible(el)) return false;
                const label = `${el.textContent || ''} ${el.getAttribute('data-testid') || ''} ${el.id || ''}`.toLowerCase();
                return label.includes('resend') || label.includes('send again') || label.includes('gửi lại');
            });

            const codeInputs = Array.from(document.querySelectorAll('input')).filter((input) => {
                if (!isVisible(input)) return false;
                const identity = [
                    input.id,
                    input.name,
                    input.getAttribute('data-testid'),
                    input.getAttribute('autocomplete'),
                    input.getAttribute('placeholder'),
                    input.getAttribute('aria-label'),
                    input.closest('[data-testid]')?.getAttribute('data-testid'),
                    input.closest('label')?.textContent,
                    document.querySelector(`label[for="${CSS.escape(input.id || '')}"]`)?.textContent,
                ].filter(Boolean).join(' ').toLowerCase();

                if (/(card|cvv|exp|phone|postal|zip|billing|email|password|name|city|state|line|address)/i.test(identity)) {
                    return false;
                }

                const maxLength = Number(input.getAttribute('maxlength') || input.maxLength || 0);
                const hasCodeHint = /(otp|one-time|one time|security|verification|confirm|digit|ci-cibasic)/i.test(identity);
                return hasCodeHint || (looksLikeCode && (maxLength === 1 || maxLength === 6));
            });

            const hasCardOrBillingForm = [
                '#cardNumber',
                '#cardCvv',
                '#cardExpiry',
                '#billingLine1',
                '#billingPostalCode',
                '#phone',
                'button[data-testid="submit-button"][data-atomic-wait-intent="click_select_create_account_and_continue"]',
            ].some((selector) => isVisible(document.querySelector(selector)));
            if (hasCardOrBillingForm && !looksLikeCode && !canResend && codeInputs.length === 0) {
                return false;
            }

            return looksLikeCode && (canResend || codeInputs.length > 0);
            "#
        );
        self.eval_bool(&script).await
    }

    pub async fn is_security_challenge_page(&self) -> bool {
        let script = format!(
            "(() => {{\n{}\n{}\n}})()",
            JS_IS_VISIBLE,
            r#"
            const heading =
                document.getElementById('policyBasedSecurityHeading') ||
                document.getElementById('captchaHeading');
            const challengeForm = document.querySelector('form[action*="validatecaptcha"]');
            const challengeIframe =
                document.querySelector('iframe[name*="recaptcha" i]') ||
                document.querySelector('iframe[src*="recaptcha"]');
            const challengeContainer =
                document.getElementById('captchaComponent') ||
                document.querySelector('.ngrl-anomalydetection-div') ||
                document.querySelector('#ads-plugin .appChallengeNS');

            const challengeVisible =
                isVisible(heading) ||
                isVisible(challengeForm) ||
                isVisible(challengeIframe) ||
                isVisible(challengeContainer);

            const hasCardForm = [
                '#cardNumber',
                '#cardExpiry',
                '#cardCvv',
                '#firstName',
                '#billingLine1',
                'input[name="firstName"]',
                'input[name="cardNumber"]',
                'input[autocomplete="cc-number"]'
            ].some((selector) => isVisible(document.querySelector(selector)));

            // PayPal đôi khi gắn sẵn node captcha trong DOM dù challenge chưa active.
            if (challengeVisible) {
                if (hasCardForm && !isVisible(heading)) {
                    return false;
                }
                if (!isVisible(heading) && !isVisible(challengeIframe) && !isVisible(challengeForm)) {
                    // It's just a background container (#ads-plugin .appChallengeNS)
                    return false;
                }
                return true;
            }

            const text = (document.body?.innerText || '').toLowerCase();
            if (
                (text.includes('perform security check') ||
                 text.includes('security challenge') ||
                 text.includes('please wait while we perform security check')) &&
                !hasCardForm
            ) {
                return true;
            }

            const href = (window.location?.href || '').toLowerCase();
            return href.includes('validatecaptcha') || href.includes('/auth/validatecaptcha');
            "#
        );
        self.eval_bool(&script).await
    }

    pub async fn select_us_country(&self) -> bool {
        let res = self
            .eval_str(
                r#"(() => {
            const select =
                document.querySelector('select[data-testid="countrySelector"]') ||
                document.getElementById('country') ||
                document.querySelector('select[name="country"]');
            if (!select) return 'NO_SELECT';
            if (![...select.options].some((option) => option.value === 'US')) return 'NO_US_OPTION';
            if (select.value === 'US') return 'ALREADY_US';

            select.focus();
            const descriptor = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value');
            if (descriptor?.set) descriptor.set.call(select, 'US'); else select.value = 'US';
            select.dispatchEvent(new Event('input', { bubbles: true }));
            select.dispatchEvent(new Event('change', { bubbles: true }));
            select.dispatchEvent(new Event('blur', { bubbles: true }));

            const reactKey = Object.keys(select).find((key) => key.startsWith('__reactProps$'));
            if (reactKey && select[reactKey].onChange) {
                select[reactKey].onChange({ target: select, currentTarget: select });
            }

            return select.value === 'US' ? 'SELECTED_US' : `FAILED:${select.value}`;
        })()"#,
            )
            .await;

        match res.as_str() {
            "ALREADY_US" => true,
            "SELECTED_US" => {
                self.log("Đã chuyển Quốc gia/Khu vực sang Hoa Kỳ (US) 🇺🇸");
                tokio::time::sleep(Duration::from_secs(3)).await;
                true
            }
            other => {
                self.log(&format!("Không đổi được country sang US: {}", other));
                false
            }
        }
    }

    pub async fn inject_debug_overlay(&self) {
        let script = format!(
            r#"(() => {{
                document.title = '🔥 {email} 🔥';

                // Inject captcha hider CSS (giống hệt Chrome Extension)
                let styleEl = document.getElementById('bot-captcha-hider');
                if (!styleEl) {{
                    styleEl = document.createElement('style');
                    styleEl.id = 'bot-captcha-hider';
                    styleEl.textContent = '#captcha-standalone,.captcha-overlay,.captcha-container,.AddressAutocomplete-results,#securityChallenge,.securityChallenge,#challenge-container,[data-testid="security-challenge"]{{display:none!important;height:0!important;overflow:hidden!important}}';
                    if (document.head) {{
                        document.head.appendChild(styleEl);
                    }} else if (document.documentElement) {{
                        document.documentElement.appendChild(styleEl);
                    }}
                }}

                let d = document.getElementById('bot-overlay');
                if (!d) {{
                    d = Object.assign(document.createElement('div'), {{
                        id: 'bot-overlay',
                        textContent: '🤖 Đang xử lý: {email}',
                    }});
                    Object.assign(d.style, {{
                        position: 'fixed', top: '10px', right: '10px',
                        zIndex: '9999999', background: 'rgba(255,0,0,0.9)',
                        color: 'white', padding: '10px 15px', fontSize: '18px',
                        fontWeight: 'bold', borderRadius: '5px',
                        pointerEvents: 'none', boxShadow: '0 4px 6px rgba(0,0,0,0.3)',
                    }});
                    document.body.appendChild(d);
                }} else if (!d.textContent.includes('CAPTCHA')) {{
                    d.textContent = '🤖 Đang xử lý: {email}';
                    d.style.background = 'rgba(255,0,0,0.9)';
                }}
                return 'OK';
            }})()"#,
            email = self.email,
        );
        let _ = self.eval_str(&script).await;
    }

    pub fn log(&self, msg: &str) {
        let log_msg = format!("  → [{}] {}", self.email, msg);
        std::println!("{}", log_msg);
        let _ = self.app.emit("automation-log", log_msg);
    }
}
