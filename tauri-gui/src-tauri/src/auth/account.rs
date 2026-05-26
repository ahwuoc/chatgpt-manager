use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Account {
    pub email: String,
    pub password: String,
    #[serde(alias = "session_token")]
    pub session_token: Option<String>,
    #[serde(alias = "account_id")]
    pub account_id: Option<String>,
}

fn clean_optional(value: Option<&&str>) -> Option<String> {
    value
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(|item| item.to_string())
}

fn should_include(email: &str, selected: &HashSet<String>) -> bool {
    selected.is_empty() || selected.contains(&email.trim().to_lowercase())
}

pub(crate) fn load_accounts(selected_emails: &[String]) -> Result<Vec<Account>> {
    let content = fs::read_to_string("data/accounts_list.txt")
        .map_err(|_| anyhow!("Không tìm thấy file accounts_list.txt"))?;
    let selected: HashSet<String> = selected_emails
        .iter()
        .map(|email| email.trim().to_lowercase())
        .filter(|email| !email.is_empty())
        .collect();

    let trimmed = content.trim();
    if trimmed.starts_with('[') {
        let list = serde_json::from_str::<Vec<Account>>(trimmed)
            .map_err(|e| anyhow!("Lỗi parse accounts_list JSON: {}", e))?;
        return Ok(list
            .into_iter()
            .filter(|acc| should_include(&acc.email, &selected))
            .collect());
    }

    if trimmed.starts_with('{') {
        let account = serde_json::from_str::<Account>(trimmed)
            .map_err(|e| anyhow!("Lỗi parse account JSON: {}", e))?;
        return Ok(if should_include(&account.email, &selected) {
            vec![account]
        } else {
            Vec::new()
        });
    }

    let mut accounts = Vec::new();
    for line in trimmed.lines().filter(|line| !line.trim().is_empty()) {
        let parts: Vec<&str> = line.trim().split('|').collect();
        if parts.len() < 2 {
            continue;
        }

        let email = parts[0].trim().to_string();
        if !should_include(&email, &selected) {
            continue;
        }

        accounts.push(Account {
            email,
            password: parts[1].trim().to_string(),
            session_token: clean_optional(parts.get(2)),
            account_id: clean_optional(parts.get(3)),
        });
    }

    Ok(accounts)
}
