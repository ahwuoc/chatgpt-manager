use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, Serialize, Deserialize)]
pub struct OTPMessage {
    pub from: String,
    pub subject: String,
    pub date: String,
    pub message: Option<String>,
    pub code: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OTPApiResponse {
    pub status: bool,
    pub messages: Option<Vec<OTPMessage>>,
}

pub struct OTPService {
    api_url: String,
}

impl OTPService {
    pub fn new() -> Self {
        Self {
            api_url: "https://tools.dongvanfb.net/api/get_messages_oauth2".to_string(),
        }
    }

    pub async fn fetch_latest_otp(
        &self,
        email: &str,
        password: &str,
        refresh_token: Option<&str>,
        client_id: Option<&str>,
    ) -> Result<Option<String>, Box<dyn Error>> {
        let client = wreq::Client::new();
        let body = serde_json::json!({
            "email": email,
            "pass": password,
            "refresh_token": refresh_token.unwrap_or(""),
            "client_id": client_id.unwrap_or(""),
            "list_mail": "all"
        });

        let resp = client.post(&self.api_url).json(&body).send().await?;

        let resp_text = resp.text().await?;

        let data: OTPApiResponse = serde_json::from_str(&resp_text)?;

        if !data.status || data.messages.is_none() {
            return Ok(None);
        }

        let messages = data.messages.unwrap();
        for msg in messages.iter().take(3) {
            let from_lower = msg.from.to_lowercase();
            let subject_lower = msg.subject.to_lowercase();
            if from_lower.contains("openai")
                || subject_lower.contains("openai")
                || subject_lower.contains("verification")
                || subject_lower.contains("verify")
                || subject_lower.contains("xác minh")
            {
                if let Some(otp) = self.extract_otp(msg) {
                    return Ok(Some(otp));
                }
            }
        }

        Ok(None)
    }

    fn extract_otp(&self, msg: &OTPMessage) -> Option<String> {
        if let Some(ref c) = msg.code {
            if c.len() == 6 && c.chars().all(|x| x.is_numeric()) {
                return Some(c.clone());
            }
        }

        // Tìm trong nội dung tin nhắn
        if let Some(ref text) = msg.message {
            let re = regex::Regex::new(r"[^#\d](\d{6})[^#\d]").unwrap();
            if let Some(cap) = re.captures(text) {
                if let Some(m) = cap.get(1) {
                    return Some(m.as_str().to_string());
                }
            }

            // Tìm dự phòng nếu nằm ở đầu hoặc cuối chuỗi
            let re_backup = regex::Regex::new(r"^\d{6}$").unwrap();
            if let Some(cap) = re_backup.find(text) {
                return Some(cap.as_str().to_string());
            }
        }

        None
    }
}
