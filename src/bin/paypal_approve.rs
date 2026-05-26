use anyhow::{Result, anyhow};
use chaser_oxide::{Browser, BrowserConfig, ChaserPage, Page};
use fake::Fake;
use fake::faker::address::en::{CityName, StateAbbr, StreetName, ZipCode};
use fake::faker::name::en::{FirstName, LastName};
use futures::StreamExt;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::time::Duration;

// ── Constants ─────────────────────────────────────────────────────────────────

const PAYPAL_LINKS_FILE: &str = "results/02_paypal_approve_links.txt";
const CHECKOUT_LOG_FILE: &str = "results/03_paypal_final_checkout_links.txt";
const SUCCESS_FILE: &str = "results/04_paypal_success.txt";
const CONCURRENT_LIMIT: usize = 3;
const FLOW_TIMEOUT_SECS: u64 = 90;
const POLL_INTERVAL_SECS: u64 = 2;

// ── Fake data generators ──────────────────────────────────────────────────────

fn gen_email() -> String {
    let len = rand::random_range(10..16usize);
    let name: String = (0..len)
        .map(|_| {
            let charset = b"abcdefghijklmnopqrstuvwxyz0123456789";
            charset[rand::random_range(0..charset.len())] as char
        })
        .collect();
    format!("{}@gmail.com", name)
}

fn gen_phone() -> String {
    let area = rand::random_range(200u32..999);
    let rest: String = (0..7)
        .map(|_| char::from_digit(rand::random_range(0..10), 10).unwrap())
        .collect();
    format!("{}{}", area, rest)
}

fn get_phone_number() -> String {
    fs::read_to_string("phone.txt")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(gen_phone)
}

fn gen_password() -> String {
    const UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    const LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
    const DIGITS: &[u8] = b"0123456789";
    const SYMBOLS: &[u8] = b"!@#$%^";
    const ALL: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^";

    let mut p: Vec<u8> = vec![
        UPPER[rand::random_range(0..UPPER.len())],
        LOWER[rand::random_range(0..LOWER.len())],
        DIGITS[rand::random_range(0..DIGITS.len())],
        SYMBOLS[rand::random_range(0..SYMBOLS.len())],
    ];
    for _ in 4..12 {
        p.push(ALL[rand::random_range(0..ALL.len())]);
    }
    String::from_utf8(p).unwrap()
}

fn gen_visa_card() -> String {
    let mut digits: Vec<u32> = vec![4];
    for _ in 0..14 {
        digits.push(rand::random_range(0..10));
    }
    let sum: u32 = digits
        .iter()
        .enumerate()
        .map(|(i, &d)| {
            let mut v = d;
            if i % 2 == 0 {
                v *= 2;
            }
            if v > 9 {
                v -= 9;
            }
            v
        })
        .sum();
    digits.push((10 - (sum % 10)) % 10);
    digits
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join("")
}

fn gen_cvv() -> String {
    format!("{:03}", rand::random_range(100..999u32))
}

fn js_fill(selector: &str, val: &str) -> String {
    format!(
        r#"(() => {{
            let el = document.getElementById({sel:?})
                  || document.querySelector('[name="' + {sel:?} + '"]')
                  || document.querySelector({sel:?});
            if (!el) return 'NOT_FOUND:{sel}';
            el.focus();
            const ns = Object.getOwnPropertyDescriptor(
                el.tagName === 'SELECT' ? HTMLSelectElement.prototype : HTMLInputElement.prototype,
                'value'
            );
            if (ns?.set) ns.set.call(el, {val:?}); else el.value = {val:?};
            el.dispatchEvent(new InputEvent('input',  {{ bubbles: true, inputType: 'insertText', data: {val:?} }}));
            el.dispatchEvent(new Event('change', {{ bubbles: true }}));
            el.dispatchEvent(new Event('blur',   {{ bubbles: true }}));
            const rk = Object.keys(el).find(k => k.startsWith('__reactProps$'));
            if (rk && el[rk].onChange) el[rk].onChange({{ target: el, currentTarget: el }});
            return 'OK:' + el.value;
        }})()"#,
        sel = selector,
        val = val,
    )
}

fn js_click_btn(selector: &str) -> String {
    format!(
        r#"(() => {{
            const btn = document.querySelector({sel:?});
            if (btn && !btn.disabled) {{ btn.click(); return 'CLICKED'; }}
            return 'NOT_FOUND';
        }})()"#,
        sel = selector,
    )
}

// ── PaypalPage wrapper ────────────────────────────────────────────────────────

struct PaypalPage<'a> {
    page: &'a Page,
    email: &'a str,
}

impl<'a> PaypalPage<'a> {
    fn new(page: &'a Page, email: &'a str) -> Self {
        Self { page, email }
    }

    async fn eval_str(&self, script: &str) -> String {
        self.page
            .evaluate(script)
            .await
            .ok()
            .and_then(|v| v.into_value::<String>().ok())
            .unwrap_or_default()
    }

    async fn eval_bool(&self, script: &str) -> bool {
        self.page
            .evaluate(script)
            .await
            .ok()
            .and_then(|v| v.into_value::<bool>().ok())
            .unwrap_or(false)
    }

    async fn fill(&self, selector: &str, val: &str) {
        let _ = self.eval_str(&js_fill(selector, val)).await;
    }

    async fn click(&self, selector: &str) -> bool {
        self.eval_str(&js_click_btn(selector)).await == "CLICKED"
    }

    async fn wait_for_id(&self, id: &str, timeout_secs: u64) -> bool {
        let script = format!(r#"!!document.getElementById({id:?})"#, id = id);
        let ticks = timeout_secs * 2;
        for _ in 0..ticks {
            if self.eval_bool(&script).await {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        false
    }

    async fn has_email_input(&self) -> bool {
        self.eval_str(r#"(() => {
            return !!(
                document.getElementById('login_email') ||
                document.getElementById('email') ||
                document.getElementById('onboardingFlowEmail') ||
                document.querySelector('input[type="email"], input[name="login_email"], input[name="email"]')
            );
        })()"#).await == "true"
    }

    async fn inject_debug_overlay(&self) {
        let script = format!(
            r#"(() => {{
                document.title = '🔥 {email} 🔥';
                if (document.getElementById('bot-overlay')) return 'EXISTS';
                const d = Object.assign(document.createElement('div'), {{
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
                return 'OK';
            }})()"#,
            email = self.email,
        );
        let _ = self.eval_str(&script).await;
    }

    fn log(&self, msg: &str) {
        println!("  {} [{}] {}", "→", self.email, msg);
    }
}

// ── Flow steps ────────────────────────────────────────────────────────────────

async fn step_agreements(pp: &PaypalPage<'_>) -> bool {
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

async fn step_fill_email(pp: &PaypalPage<'_>) -> bool {
    if !pp.has_email_input().await || pp.wait_for_id("cardNumber", 1).await {
        return false;
    }
    pp.log("Trang điền Email → bắt đầu điền...");
    let fake_email = gen_email();
    for sel in &[
        "login_email",
        "email",
        "onboardingFlowEmail",
        "input[type='email']",
    ] {
        pp.fill(sel, &fake_email).await;
    }
    tokio::time::sleep(Duration::from_millis(500)).await;

    let result = pp
        .eval_str(
            r#"(() => {
        const btn = document.querySelector(
            "button[data-atomic-wait-intent='Continue_To_Payment'], \
             button[data-testid='continueButton'], \
             button[data-testid='submit-button'], \
             button[type='submit'][name='btnNext']"
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

async fn step_fill_card(
    pp: &PaypalPage<'_>,
    url: &str,
) -> bool {
    if !pp.wait_for_id("cardNumber", 1).await {
        // Cố gắng mở form điền thẻ nếu bị ẩn
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
        return false;
    }

    pp.log("Tìm thấy #cardNumber → bắt đầu điền form thẻ 💳");

    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(CHECKOUT_LOG_FILE)
    {
        let _ = writeln!(file, "{}|{}", pp.email, url);
    }

    let card_num = gen_visa_card();
    let card_expiry = "12 / 30";
    let cvv = gen_cvv();
    let first: String = FirstName().fake();
    let last_name: String = LastName().fake();
    let street: String = StreetName().fake();
    let city: String = CityName().fake();
    let state: String = StateAbbr().fake();
    let zip: String = {
        let z: String = ZipCode().fake();
        z[..z.len().min(5)].to_string()
    };
    let phone = get_phone_number();
    let pass = gen_password();

    println!(
        "  💳 [{}] Thẻ: {} / {} / {}",
        pp.email, card_num, card_expiry, cvv
    );

    let fields: &[(&str, &str)] = &[
        ("cardNumber", &card_num),
        ("cardExpiry", card_expiry),
        ("cardCvv", &cvv),
        ("firstName", &first),
        ("lastName", &last_name),
        ("billingLine1", &street),
        ("billingCity", &city),
        ("billingState", &state),
        ("billingPostalCode", &zip),
        ("phone", &phone),
        ("password", &pass),
    ];

    for (id, val) in fields {
        pp.fill(id, val).await;
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    pp.log("Đã điền xong toàn bộ form thẻ ✅");

    tokio::time::sleep(Duration::from_secs(1)).await;
    let submitted = pp.eval_str(r#"(() => {
        const btn = document.querySelector('button[data-testid="submit-button"]')
                 || document.querySelector('button[data-atomic-wait-intent="click_select_create_account_and_continue"]')
                 || document.querySelector('button[type="submit"]');
        if (btn && !btn.disabled) { btn.click(); return 'CLICKED'; }
        return 'NOT_FOUND';
    })()"#).await;

    submitted == "CLICKED"
}

async fn step_consent(pp: &PaypalPage<'_>) -> bool {
    let res = pp
        .eval_str(
            r#"(() => {
        const btn = document.getElementById("consentButton");
        if (btn && !btn.disabled) { btn.click(); return 'CLICKED_CONSENT'; }
        return 'NOT_FOUND';
    })()"#,
        )
        .await;

    res == "CLICKED_CONSENT"
}

// ── Core automation loop ──────────────────────────────────────────────────────

struct FlowState {
    agreement_clicked: bool,
    email_filled: bool,
    card_filled: bool,
    consent_clicked: bool,
}

async fn run_approval_flow(chaser: &ChaserPage, email: &str) -> Result<()> {
    let raw = chaser.raw_page();
    let pp = PaypalPage::new(raw, email);

    pp.log("Phân tích trang PayPal...");

    let mut state = FlowState {
        agreement_clicked: false,
        email_filled: false,
        card_filled: false,
        consent_clicked: false,
    };
    let mut last_logged_url = String::new();

    let flow = async {
        loop {
            let url = chaser.url().await.unwrap_or(None).unwrap_or_default();
            pp.inject_debug_overlay().await;

            // Nếu đổi URL (ví dụ chuyển từ trang email sang điền card), reset các flag tương ứng để cho phép điền tiếp
            if url != last_logged_url {
                pp.log(&format!("Chuyển URL: {}…", &url[..url.len().min(80)]));
                last_logged_url = url.clone();
            }

            let is_pay_page = ["/pay", "/signin", "checkoutweb", "hermes"]
                .iter()
                .any(|p| url.contains(p));

            // ── Giai đoạn 1: Đồng ý điều khoản ───────────────────────────────
            if url.contains("agreements/approve") && !state.agreement_clicked {
                if step_agreements(&pp).await {
                    state.agreement_clicked = true;
                }
            }

            // ── Giai đoạn 2: Điền form email ────────────────────────────────
            if is_pay_page && !state.email_filled {
                if step_fill_email(&pp).await {
                    state.email_filled = true;
                }
            }

            // ── Giai đoạn 3: Điền form thẻ ──────────────────────────────────
            // Chỉ điền khi tìm thấy input cardNumber và chưa điền thành công
            if is_pay_page && pp.wait_for_id("cardNumber", 1).await && !state.card_filled {
                if step_fill_card(&pp, &url).await {
                    pp.log("Submit thẻ → THÀNH CÔNG ✅");
                    state.card_filled = true;
                    return Ok::<(), anyhow::Error>(());
                }
            }

            // ── Giai đoạn 4: Agree & Continue (Consent) ─────────────────────
            if is_pay_page && !state.consent_clicked {
                if step_consent(&pp).await {
                    pp.log("Nhấn 'Agree and Continue' → HOÀN TẤT ✅");
                    state.consent_clicked = true;
                    if let Ok(mut f) = fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(SUCCESS_FILE)
                    {
                        let _ = writeln!(f, "{}", email);
                    }
                    return Ok(());
                }
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
            Ok(())
        }
    }
}

// ── Browser lifecycle ─────────────────────────────────────────────────────────

async fn process_link(email: &str, url: &str, slot: usize) -> Result<()> {
    println!("\n{}", "─".repeat(60));
    println!(
        "🚀 [Slot {}] {} | {}…",
        slot,
        email,
        &url[..url.len().min(80)]
    );

    let window_w: u32 = 900;
    let window_h: u32 = 900;
    let window_x = 50 + slot as u32 * (window_w + 20);

    let profile_dir = format!(
        "./chrome_profiles/{}",
        email.replace('@', "_").replace('.', "_")
    );

    let config = BrowserConfig::builder()
        .with_head()
        .window_size(window_w, window_h)
        .arg(format!("--window-position={},{}", window_x, 50))
        .user_data_dir(profile_dir)
        .build()
        .map_err(|e| anyhow!("Lỗi cấu hình browser: {}", e))?;

    let (browser, mut handler) = Browser::launch(config).await?;
    tokio::spawn(async move { while handler.next().await.is_some() {} });

    let page = browser.new_page("about:blank").await?;
    let chaser = ChaserPage::new(page);
    chaser.apply_native_profile().await?;

    println!("  🌐 [{}] Mở URL PayPal…", email);
    chaser.goto(url).await?;
    tokio::time::sleep(Duration::from_secs(5)).await;

    run_approval_flow(&chaser, email).await?;

    println!("  ✅ [{}] Hoàn tất, đóng trình duyệt.", email);
    drop(browser);
    Ok(())
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 PAYPAL APPROVE — Tự động điền form thanh toán PayPal");

    let content = fs::read_to_string(PAYPAL_LINKS_FILE)
        .map_err(|_| anyhow!("❌ Không tìm thấy {}", PAYPAL_LINKS_FILE))?;

    let success_emails: HashSet<String> = fs::read_to_string(SUCCESS_FILE)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect();

    let links: Vec<(String, String)> = content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let (email, url) = line.split_once('|')?;
            if success_emails.contains(email) {
                return None;
            }
            Some((email.to_string(), url.to_string()))
        })
        .collect();

    if links.is_empty() {
        println!(
            "❌ Không có link nào cần xử lý trong {}.",
            PAYPAL_LINKS_FILE
        );
        return Ok(());
    }

    println!(
        "📋 Tìm thấy {} link — chạy {} luồng song song.",
        links.len(),
        CONCURRENT_LIMIT
    );

    futures::stream::iter(links.into_iter().enumerate())
        .for_each_concurrent(CONCURRENT_LIMIT, |(idx, (email, url))| async move {
            let slot = idx % CONCURRENT_LIMIT;
            if let Err(e) = process_link(&email, &url, slot).await {
                println!("❌ Lỗi slot {}: {}", slot, e);
            }
        })
        .await;

    println!("\n✨ HOÀN TẤT!");
    Ok(())
}
