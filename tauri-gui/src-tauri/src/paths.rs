pub const ACCOUNTS_LIST_FILE: &str = "data/accounts_list.txt";
pub const ACCESS_TOKENS_FILE: &str = "data/access_tokens.txt";
pub const STRIPE_LINKS_FILE: &str = "data/results/01_stripe_checkout_links.jsonl";
pub const PAYPAL_LINKS_FILE: &str = "data/results/02_paypal_approve_links.txt";
pub const CHECKOUT_LOG_FILE: &str = "data/results/03_paypal_final_checkout_links.txt";
pub const PHONE_FILE: &str = "data/phone.txt";
pub const SUCCESS_FILE: &str = "data/results/04_paypal_success.txt";
pub const TRIAL_REGISTERED_FILE: &str = "data/trial_registered.json";
pub const PLUS_VERIFIED_REAL_FILE: &str = "data/results/05_plus_verified_real.txt";
pub const CHROME_PROFILES_DIR: &str = "data/chrome_profiles";
pub const PROFILE_RUN_IPS_FILE: &str = "data/profile_run_ips.json";

/// Timeout cho flow PayPal approve (giây)
pub const FLOW_TIMEOUT_SECS: u64 = 900;
/// Khoảng cách giữa các vòng poll trong flow (giây)
pub const POLL_INTERVAL_SECS: u64 = 2;
