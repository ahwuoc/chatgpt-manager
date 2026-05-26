use crate::otp;
use std::collections::HashMap;
use std::fs;

const ACCOUNTS_LIST_FILE: &str = "data/accounts_list.txt";

#[derive(Clone)]
pub struct MailAccount {
    pub email: String,
    pub password: String,
    pub refresh_token: String,
    pub client_id: String,
}

pub fn read_mail_account_map() -> HashMap<String, MailAccount> {
    let mut accounts = HashMap::new();
    if let Ok(content) = fs::read_to_string(ACCOUNTS_LIST_FILE) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() < 2 {
                continue;
            }

            let email = parts[0].trim().to_lowercase();
            if email.is_empty() {
                continue;
            }

            accounts.insert(
                email.clone(),
                MailAccount {
                    email,
                    password: parts[1].trim().to_string(),
                    refresh_token: parts
                        .get(2)
                        .map(|s| s.trim())
                        .unwrap_or_default()
                        .to_string(),
                    client_id: parts
                        .get(3)
                        .map(|s| s.trim())
                        .unwrap_or_default()
                        .to_string(),
                },
            );
        }
    }
    accounts
}

#[tauri::command]
pub async fn get_otp(email: String, pass: String) -> Result<String, String> {
    let account_map = read_mail_account_map();
    let email_key = email.trim().to_lowercase();
    let account = account_map
        .get(&email_key)
        .ok_or_else(|| "Không tìm thấy account trong data/accounts_list.txt.".to_string())?;

    let password = if account.password.is_empty() {
        pass.as_str()
    } else {
        account.password.as_str()
    };

    let otp_service = otp::OTPService::new();
    otp_service
        .fetch_latest_otp_after(
            &account.email,
            password,
            if account.refresh_token.is_empty() {
                None
            } else {
                Some(account.refresh_token.as_str())
            },
            if account.client_id.is_empty() {
                None
            } else {
                Some(account.client_id.as_str())
            },
            None,
        )
        .await
        .map_err(|e| format!("Lỗi Microsoft Graph: {}", e))?
        .ok_or_else(|| "Không tìm thấy mã OTP OpenAI trong Microsoft mailbox.".to_string())
}
