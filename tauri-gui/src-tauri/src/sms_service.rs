use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SmsConfig {
    #[serde(default)]
    pub manual_phone: String,
    #[serde(default)]
    pub otp_relay_url: String,
}

impl Default for SmsConfig {
    fn default() -> Self {
        Self {
            manual_phone: String::new(),
            otp_relay_url: "https://mail-api.yuecheng.shop/api/text-relay/eca_tr_DWLd3xXapmgvHPLyOxsCUXOy".to_string(),
        }
    }
}

pub struct SmsService {
    pub config: SmsConfig,
}

impl SmsService {
    pub fn new() -> Self {
        Self {
            config: Self::load_config(),
        }
    }

    pub fn load_config() -> SmsConfig {
        let path = "data/sms_config.json";
        if !Path::new(path).exists() {
            let default_config = SmsConfig::default();
            let _ = Self::save_config(&default_config);
            return default_config;
        }

        match fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|_| SmsConfig::default()),
            Err(_) => SmsConfig::default(),
        }
    }

    pub fn save_config(config: &SmsConfig) -> Result<(), String> {
        let path = "data/sms_config.json";
        if let Some(parent) = Path::new(path).parent() {
            let _ = fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
        fs::write(path, json).map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn get_sms_config() -> Result<SmsConfig, String> {
    Ok(SmsService::load_config())
}

#[tauri::command]
pub fn save_sms_config(config: SmsConfig) -> Result<(), String> {
    SmsService::save_config(&config)
}
