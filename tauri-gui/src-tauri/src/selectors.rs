use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PaypalSelectors {
    pub submit_button: String,
    pub country_selector: String,
    pub email_input: String,
    pub consent_button: String,
    pub add_fi_link: String,
    pub exceed_error: String,
    pub otp_input: String,
}

impl Default for PaypalSelectors {
    fn default() -> Self {
        Self {
            submit_button: "button[data-testid=\"submit-button\"], button[data-atomic-wait-intent=\"click_select_create_account_and_continue\"], button[type=\"submit\"]".to_string(),
            country_selector: "select[data-testid=\"countrySelector\"], #country, select[name=\"country\"]".to_string(),
            email_input: "#email, input[name=\"email\"], input[type=\"email\"]".to_string(),
            consent_button: "button[data-testid='consentButton']".to_string(),
            add_fi_link: "[data-testid=\"add-fi-link\"] button".to_string(),
            exceed_error: "[data-testid=\"exceed-main\"], [data-testid=\"primary-button-exceed\"]".to_string(),
            otp_input: "[data-testid=\"sca-confirm-multi-field\"], [id*=\"ci-\"], input[autocomplete=\"one-time-code\"]".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SelectorConfig {
    pub paypal: PaypalSelectors,
}

impl SelectorConfig {
    pub fn load() -> Self {
        let path = "data/selectors.json";
        if !Path::new(path).exists() {
            let default_config = Self::default();
            let _ = Self::save(&default_config);
            return default_config;
        }

        match fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str::<Self>(&content) {
                Ok(config) => config,
                Err(_) => Self::default(),
            },
            Err(_) => Self::default(),
        }
    }

    pub fn save(config: &Self) -> Result<(), String> {
        let path = "data/selectors.json";
        if let Some(parent) = Path::new(path).parent() {
            let _ = fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
        fs::write(path, json).map_err(|e| e.to_string())
    }
}
