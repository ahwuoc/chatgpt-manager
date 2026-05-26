use serde::{Deserialize, Serialize};

const MICROSOFT_TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const MICROSOFT_GRAPH_MESSAGES_URL: &str = "https://graph.microsoft.com/v1.0/me/messages?$select=subject,from,receivedDateTime,bodyPreview&$orderby=receivedDateTime%20desc&$top=25";

#[derive(Debug, Serialize, Deserialize)]
pub struct OTPMessage {
    pub from: String,
    pub subject: String,
    pub date: String,
    pub message: Option<String>,
    pub code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MicrosoftTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphEmailAddress {
    address: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphRecipient {
    #[serde(rename = "emailAddress")]
    email_address: Option<GraphEmailAddress>,
}

#[derive(Debug, Deserialize)]
struct GraphMessage {
    subject: Option<String>,
    from: Option<GraphRecipient>,
    #[serde(rename = "receivedDateTime")]
    received_date_time: Option<String>,
    #[serde(rename = "bodyPreview")]
    body_preview: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphMessagesResponse {
    value: Option<Vec<GraphMessage>>,
}

pub struct OTPService;

impl OTPService {
    pub fn new() -> Self {
        Self
    }

    pub async fn fetch_latest_otp_after(
        &self,
        email: &str,
        password: &str,
        refresh_token: Option<&str>,
        client_id: Option<&str>,
        not_before_ts: Option<i64>,
    ) -> Result<Option<String>, anyhow::Error> {
        let messages = self
            .fetch_messages(email, password, refresh_token, client_id)
            .await?;

        let mut latest: Option<(usize, i64, String)> = None;

        for (idx, msg) in messages.iter().enumerate() {
            let from_lower = msg.from.to_lowercase();
            let subject_lower = msg.subject.to_lowercase();

            if from_lower.contains("openai")
                || subject_lower.contains("openai")
                || subject_lower.contains("verification")
                || subject_lower.contains("verify")
                || subject_lower.contains("xác minh")
            {
                let ts = parse_message_timestamp(&msg.date);
                if let Some(not_before_ts) = not_before_ts {
                    if !ts.is_some_and(|ts| ts >= not_before_ts) {
                        continue;
                    }
                }

                if let Some(otp) = self.extract_otp(msg) {
                    let ts = ts.unwrap_or(i64::MIN);
                    match &latest {
                        Some((best_idx, best_ts, _)) => {
                            if ts > *best_ts
                                || (ts == *best_ts && ts == i64::MIN && idx < *best_idx)
                            {
                                latest = Some((idx, ts, otp));
                            }
                        }
                        None => latest = Some((idx, ts, otp)),
                    }
                }
            }
        }

        Ok(latest.map(|(_, _, otp)| otp))
    }

    pub async fn fetch_messages(
        &self,
        email: &str,
        _password: &str,
        refresh_token: Option<&str>,
        client_id: Option<&str>,
    ) -> Result<Vec<OTPMessage>, anyhow::Error> {
        let refresh_token = non_empty(refresh_token).ok_or_else(|| {
            anyhow::anyhow!("{} thiếu refresh_token để gọi Microsoft Graph", email)
        })?;
        let client_id = non_empty(client_id)
            .ok_or_else(|| anyhow::anyhow!("{} thiếu client_id để gọi Microsoft Graph", email))?;

        self.fetch_messages_from_microsoft(refresh_token, client_id)
            .await
    }

    async fn fetch_messages_from_microsoft(
        &self,
        refresh_token: &str,
        client_id: &str,
    ) -> Result<Vec<OTPMessage>, anyhow::Error> {
        let client = wreq::Client::new();
        let body = form_urlencoded(&[
            ("client_id", client_id),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            (
                "scope",
                "offline_access https://graph.microsoft.com/Mail.Read",
            ),
        ]);

        let token_resp = client
            .post(MICROSOFT_TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Lỗi gửi request token Microsoft: {}", e))?;

        let token_status = token_resp.status();
        let token_text = token_resp
            .text()
            .await
            .map_err(|e| anyhow::anyhow!("Lỗi đọc token Microsoft: {}", e))?;
        let token_json: MicrosoftTokenResponse = serde_json::from_str(&token_text)
            .map_err(|e| anyhow::anyhow!("Lỗi parse token Microsoft: {}", e))?;

        let access_token = token_json.access_token.ok_or_else(|| {
            anyhow::anyhow!(
                "Không đổi được Microsoft access token: {}",
                token_json
                    .error_description
                    .or(token_json.error)
                    .unwrap_or_else(|| format!("HTTP_STATUS_{}", token_status.as_u16()))
            )
        })?;

        let graph_resp = client
            .get(MICROSOFT_GRAPH_MESSAGES_URL)
            .header("Authorization", &format!("Bearer {}", access_token))
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Lỗi gửi request Microsoft Graph: {}", e))?;

        let graph_status = graph_resp.status();
        if !graph_status.is_success() {
            return Err(anyhow::anyhow!(
                "MICROSOFT_GRAPH_HTTP_{}",
                graph_status.as_u16()
            ));
        }

        let graph_text = graph_resp
            .text()
            .await
            .map_err(|e| anyhow::anyhow!("Lỗi đọc Microsoft Graph: {}", e))?;
        let graph_json: GraphMessagesResponse = serde_json::from_str(&graph_text)
            .map_err(|e| anyhow::anyhow!("Lỗi parse Microsoft Graph: {}", e))?;

        Ok(graph_json
            .value
            .unwrap_or_default()
            .into_iter()
            .map(|msg| {
                let from = msg
                    .from
                    .and_then(|from| from.email_address)
                    .and_then(|email| email.address.or(email.name))
                    .unwrap_or_default();
                let subject = msg.subject.unwrap_or_default();
                let preview = msg.body_preview.unwrap_or_default();

                OTPMessage {
                    from,
                    subject,
                    date: msg.received_date_time.unwrap_or_default(),
                    code: extract_6_digit_otp(&preview),
                    message: Some(preview),
                }
            })
            .collect())
    }

    fn extract_otp(&self, msg: &OTPMessage) -> Option<String> {
        if let Some(ref c) = msg.code {
            if c.len() == 6 && c.chars().all(|x| x.is_numeric()) {
                return Some(c.clone());
            }
        }

        if let Some(ref text) = msg.message {
            if let Some(otp) = extract_6_digit_otp(text) {
                return Some(otp);
            }
        }

        None
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.and_then(|value| {
        let value = value.trim();
        if value.is_empty() {
            None
        } else {
            Some(value)
        }
    })
}

fn form_urlencoded(params: &[(&str, &str)]) -> String {
    params
        .iter()
        .map(|(key, value)| {
            format!(
                "{}={}",
                urlencoding::encode(key),
                urlencoding::encode(value)
            )
        })
        .collect::<Vec<_>>()
        .join("&")
}

pub fn extract_6_digit_otp(text: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?m)(^|[^\d#])(\d{6})([^\d#]|$)").unwrap();
    re.captures(text)
        .and_then(|cap| cap.get(2))
        .map(|m| m.as_str().to_string())
}

fn parse_message_timestamp(date: &str) -> Option<i64> {
    let date = date.trim();
    if date.is_empty() {
        return None;
    }

    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(date) {
        return Some(dt.timestamp());
    }

    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(date) {
        return Some(dt.timestamp());
    }

    for fmt in [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%d/%m/%Y %H:%M:%S",
        "%d/%m/%Y %H:%M",
    ] {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(date, fmt) {
            return Some(dt.and_utc().timestamp());
        }
    }

    None
}
