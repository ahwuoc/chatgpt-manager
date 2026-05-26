use chaser_oxide::Page;
use std::time::Duration;
use tauri::Emitter;

pub fn js_fill(selector: &str, val: &str) -> String {
    format!(
        r#"(() => {{
            let findEl = (doc, sel) => {{
                if (!doc) return null;
                let el = doc.getElementById(sel) || doc.querySelector('[name="' + sel + '"]') || doc.querySelector(sel);
                if (el) return el;
                let frames = doc.querySelectorAll('iframe');
                for (let i = 0; i < frames.length; i++) {{
                    try {{ el = findEl(frames[i].contentDocument, sel); if (el) return el; }} catch(e) {{}}
                }}
                return null;
            }};
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
        sel = selector,
        val = val,
    )
}

pub fn js_click_btn(selector: &str) -> String {
    format!(
        r#"(() => {{
            let findEl = (doc, sel) => {{
                if (!doc) return null;
                let el = doc.querySelector(sel);
                if (el) return el;
                let frames = doc.querySelectorAll('iframe');
                for (let i = 0; i < frames.length; i++) {{
                    try {{ el = findEl(frames[i].contentDocument, sel); if (el) return el; }} catch(e) {{}}
                }}
                return null;
            }};
            let btn = findEl(document, {sel:?});
            if (btn && !btn.disabled) {{ btn.click(); return 'CLICKED'; }}
            return 'NOT_FOUND';
        }})()"#,
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
            r#"(() => {{
                let findEl = (doc, sel) => {{
                    if (!doc) return false;
                    if (doc.getElementById(sel) || doc.querySelector('[name="' + sel + '"]')) return true;
                    let frames = doc.querySelectorAll('iframe');
                    for (let i = 0; i < frames.length; i++) {{
                        try {{ if (findEl(frames[i].contentDocument, sel)) return true; }} catch(e) {{}}
                    }}
                    return false;
                }};
                return findEl(document, {id:?});
            }})()"#,
            id = id
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
        self.eval_bool(r#"(() => {
            const isVisible = (el) => {
                if (!el) return false;
                const rect = el.getBoundingClientRect();
                const style = window.getComputedStyle(el);
                return rect.width > 0 &&
                       rect.height > 0 &&
                       style.display !== 'none' &&
                       style.visibility !== 'hidden';
            };
            const el = document.getElementById('login_email') ||
                       document.getElementById('email') ||
                       document.getElementById('onboardingFlowEmail') ||
                       document.querySelector('input[type="email"], input[name="login_email"], input[name="email"]');
            return isVisible(el);
        })()"#).await
    }

    pub async fn is_initial_create_account_prompt(&self) -> bool {
        self.eval_bool(
            r#"(() => {
            const submitForm = document.getElementById('publicCredentialSubmitForm');
            const createForm = document.querySelector('form[data-testid="create-account-form"]');
            const createBtn = createForm?.querySelector(
                "button[data-atomic-wait-task='login_create_account'], button[type='submit']"
            );
            const createAccountEmailForm = document.querySelector('form[data-testid="emailForm"]');
            
            const isVisible = (el) => {
                if (!el) return false;
                const rect = el.getBoundingClientRect();
                const style = window.getComputedStyle(el);
                return rect.width > 0 &&
                       rect.height > 0 &&
                       style.display !== 'none' &&
                       style.visibility !== 'hidden';
            };
            const emailEl = document.getElementById('login_email') ||
                            document.getElementById('email') ||
                            document.querySelector('input[type="email"]');
            const hasEmail = isVisible(emailEl);
            
            return !!submitForm && !!createForm && !!createBtn && !createAccountEmailForm && !hasEmail;
        })()"#,
        )
        .await
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
        self.eval_bool(
            r#"(() => {
            const isVisible = (el) => {
                if (!el) return false;
                const rect = el.getBoundingClientRect();
                const style = window.getComputedStyle(el);
                return rect.width > 0 &&
                       rect.height > 0 &&
                       style.display !== 'none' &&
                       style.visibility !== 'hidden' &&
                       style.opacity !== '0';
            };

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
        })()"#,
        )
        .await
    }

    pub async fn is_security_challenge_page(&self) -> bool {
        self.eval_bool(
            r#"(() => {
            const isVisible = (el) => {
                if (!el) return false;
                const rect = el.getBoundingClientRect();
                const style = window.getComputedStyle(el);
                return rect.width > 0 &&
                       rect.height > 0 &&
                       style.display !== 'none' &&
                       style.visibility !== 'hidden' &&
                       style.opacity !== '0';
            };

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
        })()"#,
        )
        .await
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

    pub async fn inject_api_sniffer(&self) {
        let script = r#"(() => {
            if (window.__api_interceptor_loaded) return 'ALREADY_LOADED';
            window.__api_interceptor_loaded = true;
            window.__api_logs = [];

            // Hook Fetch
            const originalFetch = window.fetch;
            window.fetch = async function(...args) {
                const url = args[0];
                const options = args[1] || {};
                const method = options.method || 'GET';
                
                let reqBody = '';
                if (options.body) {
                    try {
                        if (typeof options.body === 'string') {
                            reqBody = options.body;
                        } else if (options.body instanceof Blob) {
                            reqBody = await options.body.text();
                        } else if (options.body instanceof URLSearchParams) {
                            reqBody = options.body.toString();
                        } else {
                            reqBody = JSON.stringify(options.body);
                        }
                    } catch (e) {}
                }

                const logEntry = {
                    type: 'fetch',
                    timestamp: new Date().toISOString(),
                    url: typeof url === 'string' ? url : (url.url || ''),
                    method: method,
                    request_body: reqBody,
                    response_status: null,
                    response_body: ''
                };
                window.__api_logs.push(logEntry);

                try {
                    const response = await originalFetch(...args);
                    const clone = response.clone();
                    logEntry.response_status = response.status;
                    
                    try {
                        const text = await clone.text();
                        logEntry.response_body = text.substring(0, 10000);
                    } catch (e) {}
                    
                    return response;
                } catch (err) {
                    logEntry.response_body = 'ERROR: ' + err.message;
                    throw err;
                }
            };

            // Hook XMLHttpRequest
            const originalOpen = XMLHttpRequest.prototype.open;
            const originalSend = XMLHttpRequest.prototype.send;

            XMLHttpRequest.prototype.open = function(method, url, ...rest) {
                this.__logEntry = {
                    type: 'xhr',
                    timestamp: new Date().toISOString(),
                    url: url,
                    method: method,
                    request_body: '',
                    response_status: null,
                    response_body: ''
                };
                return originalOpen.call(this, method, url, ...rest);
            };

            XMLHttpRequest.prototype.send = function(body) {
                if (this.__logEntry) {
                    if (body) {
                        try {
                            if (typeof body === 'string') {
                                this.__logEntry.request_body = body;
                            } else if (body instanceof Blob) {
                                body.text().then(t => { this.__logEntry.request_body = t; });
                            } else {
                                this.__logEntry.request_body = JSON.stringify(body);
                            }
                        } catch(e) {}
                    }
                    window.__api_logs.push(this.__logEntry);

                    this.addEventListener('load', () => {
                        this.__logEntry.response_status = this.status;
                        try {
                            this.__logEntry.response_body = this.responseText.substring(0, 10000);
                        } catch(e) {}
                    });
                }
                return originalSend.call(this, body);
            };

            return 'LOADED';
        })()"#;
        let _ = self.page.evaluate(script).await;
    }

    pub async fn extract_and_save_api_logs(&self) {
        let script = r#"(() => {
            if (!window.__api_logs || window.__api_logs.length === 0) return '';
            const logs = JSON.stringify(window.__api_logs);
            window.__api_logs = []; // Drain logs
            return logs;
        })()"#;

        use std::collections::HashSet;
        use std::sync::Mutex;
        static SEEN_SIGNATURES: Mutex<Option<HashSet<String>>> = Mutex::new(None);

        if let Ok(js_val) = self.page.evaluate(script).await {
            if let Ok(logs_str) = js_val.into_value::<String>() {
                if !logs_str.is_empty() {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&logs_str) {
                        if let Some(arr) = parsed.as_array() {
                            if !arr.is_empty() {
                                let folder = format!("data/api_logs/{}", self.email);
                                let _ = std::fs::create_dir_all(&folder);

                                {
                                    let mut lock = SEEN_SIGNATURES.lock().unwrap();
                                    if lock.is_none() {
                                        *lock = Some(HashSet::new());
                                    }
                                }

                                for entry in arr {
                                    let url = entry["url"].as_str().unwrap_or("");
                                    let method = entry["method"].as_str().unwrap_or("GET");
                                    let req_body = entry["request_body"].as_str().unwrap_or("");

                                    // 1. Whitelist Check (Only capture ChatGPT, Stripe, and PayPal APIs)
                                    let is_important = url.contains("chatgpt.com/api/")
                                        || url.contains("api.stripe.com")
                                        || url.contains("paypal.com");
                                    if !is_important {
                                        continue;
                                    }

                                    // 2. Blacklist / Junk Filters
                                    let url_lower = url.to_lowercase();
                                    let is_junk = url_lower.contains(".js")
                                        || url_lower.contains(".css")
                                        || url_lower.contains(".png")
                                        || url_lower.contains(".jpg")
                                        || url_lower.contains(".woff")
                                        || url_lower.contains(".svg")
                                        || url_lower.contains(".ico")
                                        || url_lower.contains("statsig")
                                        || url_lower.contains("sentry")
                                        || url_lower.contains("amplitude")
                                        || url_lower.contains("telemetry")
                                        || url_lower.contains("analytics")
                                        || url_lower.contains("/logger")
                                        || url_lower.contains("/ts?")
                                        || url_lower.contains("/metrics")
                                        || url_lower.contains("/track");
                                    if is_junk {
                                        continue;
                                    }

                                    // 3. Deduplication Check (seen signatures)
                                    let signature = format!("{}|{}|{}", method, url, req_body);
                                    {
                                        let mut lock = SEEN_SIGNATURES.lock().unwrap();
                                        if let Some(ref mut set) = *lock {
                                            if set.contains(&signature) {
                                                continue; // Skip duplicates
                                            }
                                            set.insert(signature);
                                        }
                                    }

                                    let timestamp =
                                        entry["timestamp"].as_str().unwrap_or("unknown");
                                    let clean_time = timestamp.replace(":", "-").replace(".", "_");
                                    let filename =
                                        format!("{}/{}_{}.json", folder, clean_time, method);
                                    if let Ok(content) = serde_json::to_string_pretty(entry) {
                                        let _ = std::fs::write(&filename, content);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
