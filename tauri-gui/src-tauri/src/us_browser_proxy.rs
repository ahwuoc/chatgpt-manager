use anyhow::{anyhow, Result};
use chaser_oxide::auth::Credentials;
use chaser_oxide::browser::BrowserConfigBuilder;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tauri::Emitter;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct UsBrowserProxyConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub proxy: String,
    #[serde(default)]
    pub change_ip_url: String,
    #[serde(default)]
    pub rotate_ip_before_launch: bool,
}

impl Default for UsBrowserProxyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            label: String::new(),
            proxy: String::new(),
            change_ip_url: String::new(),
            rotate_ip_before_launch: false,
        }
    }
}

#[derive(Debug, Clone)]
struct UsBrowserProxy {
    label: String,
    host: String,
    port: u16,
    username: Option<String>,
    password: Option<String>,
    change_ip_url: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct ExitIpInfo {
    ip: Option<String>,
    country: Option<String>,
    country_code: Option<String>,
    region: Option<String>,
    city: Option<String>,
    isp: Option<String>,
    asn: Option<String>,
    is_proxy: Option<bool>,
    is_hosting: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsBrowserProxyStatus {
    enabled: bool,
    label: String,
    host: String,
    port: Option<u16>,
    current_ip: Option<String>,
    country: Option<String>,
    country_code: Option<String>,
    region: Option<String>,
    city: Option<String>,
    isp: Option<String>,
    asn: Option<String>,
    is_proxy: Option<bool>,
    is_hosting: Option<bool>,
    message: String,
    changed: bool,
    wait_seconds: Option<u64>,
    change_ip_url_configured: bool,
}

fn config_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/us_browser_proxy.json")
}

fn emit_log(app: &tauri::AppHandle, message: impl Into<String>) {
    let _ = app.emit("automation-log", message.into());
}

fn parse_proxy_line(raw: &UsBrowserProxyConfig) -> Result<UsBrowserProxy> {
    let proxy = raw.proxy.trim();
    if proxy.is_empty() {
        return Err(anyhow!("Browser proxy đang bật nhưng proxy rỗng."));
    }

    let without_scheme = proxy
        .strip_prefix("http://")
        .or_else(|| proxy.strip_prefix("https://"))
        .unwrap_or(proxy)
        .trim_end_matches('/');

    let (host, port_raw, username, password) =
        if let Some((credentials, host_port)) = without_scheme.rsplit_once('@') {
            let (user, pass) = credentials
                .split_once(':')
                .ok_or_else(|| anyhow!("Proxy URL thiếu user/pass: {}", proxy))?;
            let (host, port) = host_port
                .rsplit_once(':')
                .ok_or_else(|| anyhow!("Browser proxy thiếu host/port: {}", proxy))?;
            (host, port, Some(user.to_string()), Some(pass.to_string()))
        } else {
            let parts: Vec<&str> = without_scheme.splitn(4, ':').collect();
            match parts.as_slice() {
                [host, port] => (*host, *port, None, None),
                [host, port, user, pass] => (
                    *host,
                    *port,
                    Some((*user).to_string()),
                    Some((*pass).to_string()),
                ),
                _ => {
                    return Err(anyhow!(
                        "Browser proxy phải có dạng host:port hoặc host:port:user:pass."
                    ));
                }
            }
        };

    let port = port_raw
        .parse::<u16>()
        .map_err(|_| anyhow!("Port browser proxy không hợp lệ: {}", port_raw))?;

    if host.trim().is_empty() {
        return Err(anyhow!("Host browser proxy rỗng."));
    }

    Ok(UsBrowserProxy {
        label: raw.label.trim().to_string(),
        host: host.trim().to_string(),
        port,
        username,
        password,
        change_ip_url: if raw.change_ip_url.trim().is_empty() {
            None
        } else {
            Some(raw.change_ip_url.trim().to_string())
        },
    })
}

fn load_config_raw() -> Result<UsBrowserProxyConfig> {
    let path = config_path();
    if !path.exists() {
        return Ok(UsBrowserProxyConfig::default());
    }

    let content = fs::read_to_string(&path)
        .map_err(|e| anyhow!("Không đọc được {}: {}", path.display(), e))?;
    serde_json::from_str(&content)
        .map_err(|e| anyhow!("Không parse được {}: {}", path.display(), e))
}

fn load_us_browser_proxy() -> Result<Option<UsBrowserProxy>> {
    let raw = load_config_raw()?;
    if !raw.enabled {
        return Ok(None);
    }

    parse_proxy_line(&raw).map(Some)
}

#[tauri::command]
pub fn get_us_browser_proxy_config() -> Result<UsBrowserProxyConfig, String> {
    load_config_raw().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_us_browser_proxy_config(config: UsBrowserProxyConfig) -> Result<(), String> {
    if config.enabled {
        parse_proxy_line(&config).map_err(|e| e.to_string())?;
    }

    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let json = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())
}

fn status_from_config(
    raw: &UsBrowserProxyConfig,
    proxy: Option<&UsBrowserProxy>,
    exit_info: Option<ExitIpInfo>,
    message: String,
    changed: bool,
    wait_seconds: Option<u64>,
) -> UsBrowserProxyStatus {
    let exit_info = exit_info.unwrap_or_default();
    UsBrowserProxyStatus {
        enabled: raw.enabled,
        label: proxy
            .map(|item| item.label.clone())
            .filter(|label| !label.is_empty())
            .unwrap_or_else(|| raw.label.trim().to_string()),
        host: proxy.map(|item| item.host.clone()).unwrap_or_default(),
        port: proxy.map(|item| item.port),
        current_ip: exit_info.ip,
        country: exit_info.country,
        country_code: exit_info.country_code,
        region: exit_info.region,
        city: exit_info.city,
        isp: exit_info.isp,
        asn: exit_info.asn,
        is_proxy: exit_info.is_proxy,
        is_hosting: exit_info.is_hosting,
        message,
        changed,
        wait_seconds,
        change_ip_url_configured: raw.change_ip_url.trim().starts_with("http"),
    }
}

fn build_proxy_client(proxy: &UsBrowserProxy) -> Result<wreq::Client> {
    let proxy_url = format!("http://{}:{}", proxy.host, proxy.port);
    let mut proxy_config = wreq::Proxy::all(&proxy_url)
        .map_err(|e| anyhow!("Proxy URL không hợp lệ {}: {}", proxy_url, e))?;

    if let (Some(username), Some(password)) = (proxy.username.as_deref(), proxy.password.as_deref())
    {
        proxy_config = proxy_config.basic_auth(username, password);
    }

    wreq::Client::builder()
        .proxy(proxy_config)
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| anyhow!("Không tạo được client check IP qua proxy: {}", e))
}

fn normalize_ip(value: &str) -> Option<String> {
    value
        .split(',')
        .map(|item| item.trim().trim_matches('"'))
        .find(|item| item.parse::<std::net::IpAddr>().is_ok())
        .map(|item| item.to_string())
}

fn parse_ip_response(text: &str) -> Option<String> {
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        if let Some(ip) = value
            .get("ip")
            .and_then(Value::as_str)
            .and_then(normalize_ip)
        {
            return Some(ip);
        }
        if let Some(ip) = value
            .get("origin")
            .and_then(Value::as_str)
            .and_then(normalize_ip)
        {
            return Some(ip);
        }
        if let Some(ip) = value.as_str().and_then(normalize_ip) {
            return Some(ip);
        }
    }

    normalize_ip(text)
}

fn value_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
}

fn parse_exit_info_response(text: &str) -> Option<ExitIpInfo> {
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        let ip = value_string(&value, "query")
            .or_else(|| value_string(&value, "ip"))
            .or_else(|| value_string(&value, "origin"))
            .and_then(|item| normalize_ip(&item));

        if ip.is_some() {
            return Some(ExitIpInfo {
                ip,
                country: value_string(&value, "country"),
                country_code: value_string(&value, "countryCode"),
                region: value_string(&value, "regionName"),
                city: value_string(&value, "city"),
                isp: value_string(&value, "isp"),
                asn: value_string(&value, "as"),
                is_proxy: value.get("proxy").and_then(Value::as_bool),
                is_hosting: value.get("hosting").and_then(Value::as_bool),
            });
        }
    }

    parse_ip_response(text).map(|ip| ExitIpInfo {
        ip: Some(ip),
        ..ExitIpInfo::default()
    })
}

fn describe_exit_info(info: &ExitIpInfo) -> String {
    let mut parts = Vec::new();
    if let Some(city) = info.city.as_deref().filter(|value| !value.is_empty()) {
        parts.push(city.to_string());
    }
    if let Some(region) = info.region.as_deref().filter(|value| !value.is_empty()) {
        if !parts.iter().any(|item| item.eq_ignore_ascii_case(region)) {
            parts.push(region.to_string());
        }
    }
    if let Some(country) = info
        .country_code
        .as_deref()
        .or(info.country.as_deref())
        .filter(|value| !value.is_empty())
    {
        if !parts.iter().any(|item| item.eq_ignore_ascii_case(country)) {
            parts.push(country.to_string());
        }
    }

    if parts.is_empty() {
        String::new()
    } else {
        format!(" ({})", parts.join(", "))
    }
}

async fn fetch_exit_info(proxy: &UsBrowserProxy) -> Result<ExitIpInfo> {
    let client = build_proxy_client(proxy)?;
    let endpoints = [
        "http://ip-api.com/json/?fields=status,message,query,country,countryCode,regionName,city,isp,as,proxy,hosting",
        "https://api.ipify.org?format=json",
        "https://httpbin.org/ip",
    ];
    let mut last_error = String::new();

    for endpoint in endpoints {
        match client.get(endpoint).send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let text = resp.text().await.unwrap_or_default();
                if let Some(info) = parse_exit_info_response(&text) {
                    return Ok(info);
                }
                last_error = format!("{} trả HTTP {} nhưng không đọc được IP.", endpoint, status);
            }
            Err(e) => {
                last_error = format!("{} lỗi: {}", endpoint, e);
            }
        }
    }

    Err(anyhow!(
        "{}",
        if last_error.is_empty() {
            "Không check được exit IP qua proxy."
        } else {
            last_error.as_str()
        }
    ))
}

fn extract_wait_seconds(message: &str) -> Option<u64> {
    let mut digits = String::new();
    for ch in message.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else if !digits.is_empty() {
            break;
        }
    }

    digits.parse::<u64>().ok()
}

fn compact_response_preview(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= 180 {
        trimmed.to_string()
    } else {
        format!("{}...", trimmed.chars().take(180).collect::<String>())
    }
}

fn parse_change_ip_response(http_status: u16, text: &str) -> (bool, String, Option<u64>) {
    let mut changed = (200..300).contains(&http_status);
    let mut message = format!(
        "ENODE trả HTTP {}: {}",
        http_status,
        compact_response_preview(text)
    );
    let mut wait_seconds = None;

    if let Ok(value) = serde_json::from_str::<Value>(text) {
        let status = value
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let status_code = value.get("statusCode").and_then(Value::as_u64);

        changed = status.eq_ignore_ascii_case("success") || status_code == Some(200);
        message = value
            .get("message")
            .or_else(|| value.get("error"))
            .and_then(Value::as_str)
            .filter(|msg| !msg.trim().is_empty())
            .unwrap_or(if changed {
                "Đổi IP thành công."
            } else {
                "ENODE chưa đổi IP."
            })
            .to_string();

        wait_seconds = value
            .get("waitSeconds")
            .or_else(|| value.get("wait_seconds"))
            .and_then(Value::as_u64)
            .or_else(|| extract_wait_seconds(&message));
    }

    (changed, message, wait_seconds)
}

#[tauri::command]
pub async fn get_us_browser_proxy_status() -> Result<UsBrowserProxyStatus, String> {
    let raw = load_config_raw().map_err(|e| e.to_string())?;
    if !raw.enabled {
        return Ok(status_from_config(
            &raw,
            None,
            None,
            "Browser proxy đang tắt.".to_string(),
            false,
            None,
        ));
    }

    let proxy = parse_proxy_line(&raw).map_err(|e| e.to_string())?;
    let (exit_info, message) = match fetch_exit_info(&proxy).await {
        Ok(info) => {
            let ip = info.ip.clone().unwrap_or_else(|| "unknown".to_string());
            let location = describe_exit_info(&info);
            (
                Some(info),
                format!("Exit IP hiện tại qua proxy: {}{}", ip, location),
            )
        }
        Err(e) => (None, format!("Không check được exit IP qua proxy: {}", e)),
    };

    Ok(status_from_config(
        &raw,
        Some(&proxy),
        exit_info,
        message,
        false,
        None,
    ))
}

#[tauri::command]
pub async fn change_us_browser_proxy_ip() -> Result<UsBrowserProxyStatus, String> {
    let raw = load_config_raw().map_err(|e| e.to_string())?;
    if !raw.enabled {
        return Err("Browser proxy đang tắt, bật proxy trước khi đổi IP.".to_string());
    }

    let proxy = parse_proxy_line(&raw).map_err(|e| e.to_string())?;
    let Some(change_ip_url) = proxy.change_ip_url.as_deref() else {
        return Err("Browser proxy chưa có link change IP.".to_string());
    };

    let client = wreq::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| format!("Không tạo được client đổi IP: {}", e))?;

    let resp = client
        .get(change_ip_url)
        .send()
        .await
        .map_err(|e| format!("Gọi link đổi IP lỗi: {}", e))?;
    let http_status = resp.status().as_u16();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("Không đọc được phản hồi đổi IP: {}", e))?;
    let (changed, mut message, wait_seconds) = parse_change_ip_response(http_status, &text);

    if changed {
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    let exit_info = match fetch_exit_info(&proxy).await {
        Ok(info) => {
            if changed {
                let ip = info.ip.clone().unwrap_or_else(|| "unknown".to_string());
                message = format!(
                    "{} Exit IP mới: {}{}",
                    message,
                    ip,
                    describe_exit_info(&info)
                );
            }
            Some(info)
        }
        Err(e) => {
            message = format!("{} Không check được exit IP sau khi đổi: {}", message, e);
            None
        }
    };

    Ok(status_from_config(
        &raw,
        Some(&proxy),
        exit_info,
        message,
        changed,
        wait_seconds,
    ))
}

pub fn apply_to_browser_builder(
    builder: BrowserConfigBuilder,
    app: &tauri::AppHandle,
    flow: &str,
    email: &str,
) -> Result<BrowserConfigBuilder> {
    let Some(proxy) = load_us_browser_proxy()? else {
        return Ok(builder);
    };

    let label = if proxy.label.is_empty() {
        "Browser proxy"
    } else {
        proxy.label.as_str()
    };
    emit_log(
        app,
        format!(
            "🔒 [{}] [{}] Dùng {}: {}:{}",
            flow, email, label, proxy.host, proxy.port
        ),
    );

    let proxy_server = format!("http://{}:{}", proxy.host, proxy.port);
    Ok(builder.arg(("proxy-server", proxy_server.as_str())))
}

pub async fn authenticate_page(
    page: &chaser_oxide::Page,
    app: &tauri::AppHandle,
    flow: &str,
    email: &str,
) -> Result<()> {
    let Some(proxy) = load_us_browser_proxy()? else {
        return Ok(());
    };

    let Some(username) = proxy.username else {
        return Ok(());
    };
    let Some(password) = proxy.password else {
        return Ok(());
    };

    page.authenticate(Credentials { username, password })
        .await
        .map_err(|e| anyhow!("Không set được proxy auth cho browser: {}", e))?;

    emit_log(
        app,
        format!(
            "🔐 [{}] [{}] Đã set user/pass cho browser proxy.",
            flow, email
        ),
    );
    Ok(())
}
