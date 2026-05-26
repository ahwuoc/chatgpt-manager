use crate::paths::SMSBOWER_API_BASE;
use crate::sms_service;
use std::collections::HashMap;

#[derive(serde::Serialize)]
pub struct PriceOption {
    country_id: String,
    country_name: String,
    count: u32,
    price: f64,
}

#[derive(serde::Deserialize)]
struct SmsCountryInfo {
    eng: Option<String>,
    rus: Option<String>,
    chn: Option<String>,
}

#[derive(serde::Serialize)]
pub struct ProviderPriceOption {
    provider_id: String,
    count: u32,
    price: f64,
}

fn get_country_name(id: &str) -> &str {
    match id {
        "0" => "Nga (Russia)",
        "1" => "Ukraine",
        "2" => "Kazakhstan",
        "3" => "Trung Quốc (China)",
        "4" => "Philippines",
        "5" => "Myanmar",
        "6" => "Indonesia",
        "7" => "Malaysia",
        "8" => "Kenya",
        "9" => "Tanzania",
        "10" => "Vietnam",
        "11" => "Kyrgyzstan",
        "12" => "Mỹ Virtual (USA Virtual)",
        "13" => "Israel",
        "14" => "Hong Kong",
        "15" => "Ba Lan (Poland)",
        "16" => "Anh (United Kingdom)",
        "17" => "Madagascar",
        "18" => "DR Congo",
        "19" => "Nigeria",
        "20" => "Macau",
        "21" => "Egypt",
        "22" => "Ấn Độ (India)",
        "23" => "Ireland",
        "24" => "Cambodia",
        "25" => "Laos",
        "31" => "South Africa",
        "36" => "Canada",
        "73" => "Brazil",
        "78" => "France",
        "107" => "Oman",
        "147" => "Zambia",
        "86" => "Việt Nam (Vietnam)",
        "187" => "Mỹ Real (USA Physical)",
        "1002" => "Korea",
        "1011" => "Martinique",
        _ => "Unknown country",
    }
}

fn normalize_country_name(id: &str, name: &str) -> String {
    let cleaned = name.trim();
    match id {
        "12" => "Mỹ Virtual (USA Virtual)".to_string(),
        "187" => "Mỹ Real (USA Physical)".to_string(),
        "25" if cleaned.eq_ignore_ascii_case("Lao People`s") => "Laos".to_string(),
        "79" if cleaned.eq_ignore_ascii_case("Papua new gvineya") => "Papua New Guinea".to_string(),
        _ if cleaned.is_empty() => get_country_name(id).to_string(),
        _ => cleaned.to_string(),
    }
}

async fn fetch_sms_country_names(
    client: &wreq::Client,
    api_key: &str,
) -> Result<HashMap<String, String>, String> {
    let url = format!(
        "{}?api_key={}&action=getCountries",
        SMSBOWER_API_BASE,
        urlencoding::encode(api_key),
    );

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Lỗi kết nối API countries: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Lỗi đọc phản hồi countries: {}", e))?;

    let countries: HashMap<String, SmsCountryInfo> = serde_json::from_str(&resp)
        .map_err(|e| format!("Lỗi parse JSON countries: {}. Phản hồi: {}", e, resp))?;

    Ok(countries
        .into_iter()
        .map(|(id, info)| {
            let raw_name = info
                .eng
                .as_deref()
                .or(info.rus.as_deref())
                .or(info.chn.as_deref())
                .unwrap_or_default();
            let name = normalize_country_name(&id, raw_name);
            (id, name)
        })
        .collect())
}

#[tauri::command]
pub fn get_sms_config() -> Result<sms_service::SmsConfig, String> {
    Ok(sms_service::SmsService::load_config())
}

#[tauri::command]
pub fn save_sms_config(config: sms_service::SmsConfig) -> Result<(), String> {
    sms_service::SmsService::save_config(&config)
}

#[tauri::command]
pub async fn query_sms_prices(
    api_key: String,
    service: String,
) -> Result<Vec<PriceOption>, String> {
    let url = format!(
        "{}?api_key={}&action=getPrices&service={}",
        SMSBOWER_API_BASE,
        urlencoding::encode(&api_key),
        urlencoding::encode(&service)
    );
    let client = match wreq::Client::builder()
        .emulation(wreq_util::Emulation::Chrome124)
        .build()
    {
        Ok(c) => c,
        Err(e) => return Err(format!("Lỗi khởi tạo client: {}", e)),
    };

    let country_names = fetch_sms_country_names(&client, &api_key)
        .await
        .unwrap_or_default();

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Lỗi kết nối API: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Lỗi đọc phản hồi: {}", e))?;

    let json: serde_json::Value = serde_json::from_str(&resp)
        .map_err(|e| format!("Lỗi parse JSON: {}. Phản hồi: {}", e, resp))?;

    let mut options = Vec::new();
    if let Some(obj) = json.as_object() {
        for (country_id, services) in obj {
            if let Some(service_data) = services.get(&service) {
                let count = service_data
                    .get("count")
                    .and_then(|c| c.as_u64())
                    .unwrap_or(0) as u32;
                let price = service_data
                    .get("cost")
                    .and_then(|p| p.as_f64())
                    .or_else(|| service_data.get("price").and_then(|p| p.as_f64()))
                    .unwrap_or(0.0);
                if count > 0 {
                    let country_name = country_names
                        .get(country_id)
                        .cloned()
                        .unwrap_or_else(|| get_country_name(country_id).to_string());
                    options.push(PriceOption {
                        country_id: country_id.clone(),
                        country_name: format!("{} (ID: {})", country_name, country_id),
                        count,
                        price,
                    });
                }
            }
        }
    }

    // Sort by price ascending
    options.sort_by(|a, b| {
        a.price
            .partial_cmp(&b.price)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(options)
}

#[tauri::command]
pub async fn query_sms_provider_prices(
    api_key: String,
    service: String,
    country: String,
) -> Result<Vec<ProviderPriceOption>, String> {
    let url = format!(
        "{}?api_key={}&action=getPricesV3&service={}&country={}",
        SMSBOWER_API_BASE,
        urlencoding::encode(&api_key),
        urlencoding::encode(&service),
        urlencoding::encode(&country)
    );
    let client = match wreq::Client::builder()
        .emulation(wreq_util::Emulation::Chrome124)
        .build()
    {
        Ok(c) => c,
        Err(e) => return Err(format!("Lỗi khởi tạo client: {}", e)),
    };

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Lỗi kết nối API: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Lỗi đọc phản hồi: {}", e))?;

    let json: serde_json::Value = serde_json::from_str(&resp)
        .map_err(|e| format!("Lỗi parse JSON: {}. Phản hồi: {}", e, resp))?;

    let service_data = json
        .get(&country)
        .and_then(|country_data| country_data.get(&service))
        .and_then(|value| value.as_object())
        .ok_or_else(|| {
            format!(
                "Không tìm thấy provider cho service [{}] ở country [{}]. Phản hồi: {}",
                service, country, resp
            )
        })?;

    let mut options = Vec::new();
    for (provider_id, data) in service_data {
        let count = data.get("count").and_then(|c| c.as_u64()).unwrap_or(0) as u32;
        let price = data
            .get("cost")
            .and_then(|p| p.as_f64())
            .or_else(|| data.get("price").and_then(|p| p.as_f64()))
            .unwrap_or(0.0);

        if count > 0 {
            options.push(ProviderPriceOption {
                provider_id: provider_id.clone(),
                count,
                price,
            });
        }
    }

    options.sort_by(|a, b| {
        b.count.cmp(&a.count).then_with(|| {
            a.price
                .partial_cmp(&b.price)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    Ok(options)
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SmsServiceItem {
    code: String,
    name: String,
}

#[derive(serde::Deserialize)]
struct GetServicesListResponse {
    services: Option<Vec<SmsServiceItem>>,
}

#[tauri::command]
pub async fn query_sms_services(api_key: String) -> Result<Vec<SmsServiceItem>, String> {
    let url = format!(
        "{}?api_key={}&action=getServicesList",
        SMSBOWER_API_BASE,
        urlencoding::encode(&api_key)
    );
    let client = match wreq::Client::builder()
        .emulation(wreq_util::Emulation::Chrome124)
        .build()
    {
        Ok(c) => c,
        Err(e) => return Err(format!("Lỗi khởi tạo client: {}", e)),
    };

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Lỗi kết nối API: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Lỗi đọc phản hồi: {}", e))?;

    // Parse the JSON structure
    let json: GetServicesListResponse = serde_json::from_str(&resp)
        .map_err(|e| format!("Lỗi parse JSON: {}. Phản hồi: {}", e, resp))?;

    if let Some(services) = json.services {
        Ok(services)
    } else {
        Err("Không tìm thấy danh sách dịch vụ trong phản hồi API từ SMSBower.".to_string())
    }
}
