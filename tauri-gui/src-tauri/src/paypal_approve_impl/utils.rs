use std::fs;

pub const PAYPAL_LINKS_FILE: &str = "data/results/02_paypal_approve_links.txt";
pub const CHECKOUT_LOG_FILE: &str = "data/results/03_paypal_final_checkout_links.txt";
pub const SUCCESS_FILE: &str = "data/results/04_paypal_success.txt";
pub const TRIAL_REGISTERED_FILE: &str = "data/trial_registered.json";
pub const FLOW_TIMEOUT_SECS: u64 = 900;
pub const POLL_INTERVAL_SECS: u64 = 2;

#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct TrialRegisteredState {
    pub registered: Vec<String>,
    pub sold: Vec<String>,
    pub fail: Vec<String>,
}

pub fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|item| item == value) {
        values.push(value.to_string());
    }
}

pub fn remove_email(values: &mut Vec<String>, email: &str) {
    values.retain(|item| item != email);
}

pub fn mark_trial_plus_success(email: &str) {
    let mut success_emails: Vec<String> = fs::read_to_string(SUCCESS_FILE)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect();
    push_unique(&mut success_emails, email);

    if let Some(parent) = std::path::Path::new(SUCCESS_FILE).parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(SUCCESS_FILE, success_emails.join("\n"));

    let mut trial_state = fs::read_to_string(TRIAL_REGISTERED_FILE)
        .ok()
        .and_then(|content| serde_json::from_str::<TrialRegisteredState>(&content).ok())
        .unwrap_or_default();

    push_unique(&mut trial_state.registered, email);
    remove_email(&mut trial_state.sold, email);
    remove_email(&mut trial_state.fail, email);

    if let Ok(content) = serde_json::to_string_pretty(&trial_state) {
        let _ = fs::write(TRIAL_REGISTERED_FILE, content);
    }
}

pub fn mark_trial_plus_fail(email: &str) {
    let mut trial_state = fs::read_to_string(TRIAL_REGISTERED_FILE)
        .ok()
        .and_then(|content| serde_json::from_str::<TrialRegisteredState>(&content).ok())
        .unwrap_or_default();

    push_unique(&mut trial_state.fail, email);
    remove_email(&mut trial_state.registered, email);
    remove_email(&mut trial_state.sold, email);

    if let Ok(content) = serde_json::to_string_pretty(&trial_state) {
        let _ = fs::write(TRIAL_REGISTERED_FILE, content);
    }
}
