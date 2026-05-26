use std::fs;

#[derive(Debug, Clone)]
pub(crate) struct SessionAccessToken {
    pub token: String,
    pub email: String,
}

pub(crate) async fn extract_session_access_token_once(
    page: &chaser_oxide::Page,
) -> Option<SessionAccessToken> {
    let script = r#"
        (async () => {
            try {
                const res = await fetch("https://chatgpt.com/api/auth/session", {
                    credentials: "include",
                    cache: "no-store",
                });
                const data = await res.json();
                const token = data.accessToken || data.access_token || "";
                const decodePayload = (jwt) => {
                    try {
                        const raw = jwt.split('.')[1] || '';
                        const padded = raw.replace(/-/g, '+').replace(/_/g, '/') + '='.repeat((4 - raw.length % 4) % 4);
                        return JSON.parse(atob(padded));
                    } catch (_) {
                        return {};
                    }
                };
                const payload = decodePayload(token);
                const email =
                    data.user?.email ||
                    payload?.["https://api.openai.com/profile"]?.email ||
                    "";
                return JSON.stringify({ token, email });
            } catch (e) {
                return JSON.stringify({ token: "", email: "" });
            }
        })()
    "#;

    if let Ok(js_val) = page.evaluate(script).await {
        if let Ok(raw) = js_val.into_value::<String>() {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
                let token = value.get("token").and_then(|item| item.as_str())?.trim();
                let email = value.get("email").and_then(|item| item.as_str())?.trim();
                if token.starts_with("eyJ") && token.split('.').count() == 3 && !email.is_empty() {
                    return Some(SessionAccessToken {
                        token: token.to_string(),
                        email: email.to_string(),
                    });
                }
            }
        }
    }

    None
}

pub(crate) async fn extract_session_access_token(
    page: &chaser_oxide::Page,
) -> Option<SessionAccessToken> {
    for _ in 0..8 {
        if let Some(session) = extract_session_access_token_once(page).await {
            return Some(session);
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    None
}

pub(crate) fn save_access_token(email: &str, token: &str) {
    let _ = fs::create_dir_all("data");
    let path = "data/access_tokens.txt";
    let mut replaced = false;
    let mut lines = Vec::new();

    if let Ok(content) = fs::read_to_string(path) {
        for line in content.lines() {
            if let Some((stored_email, _)) = line.split_once('|') {
                if stored_email.trim().eq_ignore_ascii_case(email) {
                    lines.push(format!("{}|{}", email, token));
                    replaced = true;
                    continue;
                }
            }
            if !line.trim().is_empty() {
                lines.push(line.to_string());
            }
        }
    }

    if !replaced {
        lines.push(format!("{}|{}", email, token));
    }

    if let Err(e) = fs::write(path, lines.join("\n")) {
        println!("❌ [{}] Lỗi ghi file access_tokens.txt: {:?}", email, e);
    }
}
