use crate::file_store::read_lines_safe;
use crate::paths::{ACCESS_TOKENS_FILE, ACCOUNTS_LIST_FILE, PLUS_VERIFIED_REAL_FILE};
use base64::{engine::general_purpose::URL_SAFE, Engine as _};
use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NineRouterImportResult {
    db_path: String,
    plus_real_count: usize,
    token_count: usize,
    imported: usize,
    updated: usize,
    skipped_no_token: usize,
    skipped_invalid_token: usize,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NineRouterScriptExportResult {
    export_dir: String,
    accounts_json: String,
    zip_path: String,
    requested: usize,
    exported: usize,
    skipped_no_token: usize,
    skipped_invalid_token: usize,
}

struct NineRouterTokenData {
    account_id: String,
    data_json: String,
}
fn is_chatgpt_access_token(token: &str) -> bool {
    let token = token.trim();
    token.starts_with("eyJ") && token.split('.').count() == 3
}

fn read_token_map() -> HashMap<String, String> {
    let mut tokens = HashMap::new();
    if let Ok(content) = fs::read_to_string(ACCESS_TOKENS_FILE) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some((email, token)) = line.split_once('|') {
                tokens.insert(email.trim().to_lowercase(), token.trim().to_string());
            }
        }
    }
    tokens
}

fn read_account_id_map() -> HashMap<String, String> {
    let mut account_ids = HashMap::new();
    if let Ok(content) = fs::read_to_string(ACCOUNTS_LIST_FILE) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 4 {
                let email = parts[0].trim().to_lowercase();
                let account_id = parts[3].trim();
                if !email.is_empty() && !account_id.is_empty() {
                    account_ids.insert(email, account_id.to_string());
                }
            }
        }
    }
    account_ids
}
fn now_iso_utc() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn decode_jwt_payload(token: &str) -> Option<serde_json::Value> {
    let payload = token.split('.').nth(1)?;
    let mut padded = payload.to_string();
    let remainder = padded.len() % 4;
    if remainder != 0 {
        padded.push_str(&"=".repeat(4 - remainder));
    }
    let bytes = URL_SAFE.decode(padded.as_bytes()).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn unix_seconds_to_iso(value: &serde_json::Value) -> Option<String> {
    let seconds = value
        .as_i64()
        .or_else(|| value.as_f64().map(|v| v as i64))?;
    chrono::DateTime::<Utc>::from_timestamp(seconds, 0)
        .map(|dt| dt.to_rfc3339_opts(SecondsFormat::Millis, true))
}

fn sanitize_connection_id(email: &str) -> String {
    let mut out = String::with_capacity(email.len());
    let mut last_was_dash = false;
    for ch in email.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            out.push('-');
            last_was_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "chatgpt-account".to_string()
    } else {
        trimmed.to_string()
    }
}

fn default_9router_db_path() -> Result<PathBuf, String> {
    let home = env::var("HOME")
        .map_err(|_| "Không xác định được thư mục HOME để tìm DB 9Router.".to_string())?;
    Ok(Path::new(&home).join(".9router/db/data.sqlite"))
}

fn make_9router_token_data(
    token: &str,
    email: &str,
    account_id_from_file: Option<&str>,
) -> Result<NineRouterTokenData, String> {
    let payload = decode_jwt_payload(token);
    let auth = payload
        .as_ref()
        .and_then(|value| value.get("https://api.openai.com/auth"))
        .and_then(|value| value.as_object());

    let account_id = auth
        .and_then(|section| section.get("chatgpt_account_id"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
        .or_else(|| {
            account_id_from_file
                .filter(|value| !value.trim().is_empty())
                .map(|value| value.trim().to_string())
        })
        .unwrap_or_else(|| format!("codex-imported-{}", sanitize_connection_id(email)));

    let plan_type = auth
        .and_then(|section| section.get("chatgpt_plan_type"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("plus");

    let expires_at = payload
        .as_ref()
        .and_then(|value| value.get("exp"))
        .and_then(unix_seconds_to_iso);

    let mut data = serde_json::json!({
        "accessToken": token,
        "testStatus": "active",
        "providerSpecificData": {
            "chatgptAccountId": account_id,
            "chatgptPlanType": plan_type,
        }
    });

    if let Some(expires_at) = expires_at {
        let expires_in = chrono::DateTime::parse_from_rfc3339(&expires_at)
            .ok()
            .map(|dt| (dt.timestamp() - Utc::now().timestamp()).max(0));
        if let Some(obj) = data.as_object_mut() {
            obj.insert(
                "expiresAt".to_string(),
                serde_json::Value::String(expires_at),
            );
            if let Some(expires_in) = expires_in {
                obj.insert(
                    "expiresIn".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(expires_in)),
                );
            }
        }
    }

    let data_json = serde_json::to_string(&data).map_err(|e| e.to_string())?;
    Ok(NineRouterTokenData {
        account_id,
        data_json,
    })
}

fn unique_connection_id(conn: &Connection, preferred_id: &str) -> Result<String, String> {
    let base = preferred_id.trim();
    let base = if base.is_empty() {
        "chatgpt-account"
    } else {
        base
    };

    for idx in 0..1000 {
        let candidate = if idx == 0 {
            base.to_string()
        } else {
            format!("{}-{}", base, idx)
        };
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM providerConnections WHERE id = ?1",
                params![candidate],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if exists == 0 {
            return Ok(candidate);
        }
    }

    Err("Không tạo được id duy nhất cho providerConnections.".to_string())
}

fn make_9router_export_account(
    email: &str,
    token: &str,
    account_id_from_file: Option<&str>,
) -> Result<serde_json::Value, String> {
    let token_data = make_9router_token_data(token, email, account_id_from_file)?;
    let mut account = serde_json::from_str::<serde_json::Value>(&token_data.data_json)
        .map_err(|e| e.to_string())?;
    let now = now_iso_utc();

    let obj = account
        .as_object_mut()
        .ok_or_else(|| "Không tạo được JSON 9Router account.".to_string())?;
    obj.insert(
        "id".to_string(),
        serde_json::Value::String(token_data.account_id),
    );
    obj.insert(
        "provider".to_string(),
        serde_json::Value::String("codex".to_string()),
    );
    obj.insert(
        "authType".to_string(),
        serde_json::Value::String("oauth".to_string()),
    );
    obj.insert(
        "name".to_string(),
        serde_json::Value::String(email.to_string()),
    );
    obj.insert(
        "email".to_string(),
        serde_json::Value::String(email.to_string()),
    );
    obj.insert(
        "priority".to_string(),
        serde_json::Value::Number(serde_json::Number::from(9)),
    );
    obj.insert("isActive".to_string(), serde_json::Value::Bool(true));
    obj.insert(
        "createdAt".to_string(),
        serde_json::Value::String(now.clone()),
    );
    obj.insert("updatedAt".to_string(), serde_json::Value::String(now));

    Ok(account)
}

fn windows_import_ps1() -> &'static str {
    r#"# Import-Windows.ps1
# Import ChatGPT/Codex accounts into 9Router SQLite database.
# Close 9Router before running this script to avoid database locks.

param (
    [string]$JsonPath = "accounts.json",
    [string]$DbPath = "$env:APPDATA\9router\db\data.sqlite"
)

Write-Host "==============================================" -ForegroundColor Green
Write-Host "      9Router Account Importer - Windows" -ForegroundColor Green
Write-Host "==============================================" -ForegroundColor Green
Write-Host "Close 9Router before running this script." -ForegroundColor Yellow
Write-Host ""

$typeDefinition = @"
using System;
using System.Runtime.InteropServices;
public class WinSQLite3 {
    [DllImport("winsqlite3.dll", EntryPoint = "sqlite3_open", CallingConvention = CallingConvention.Cdecl)]
    public static extern int Open(string filename, out IntPtr db);
    [DllImport("winsqlite3.dll", EntryPoint = "sqlite3_close", CallingConvention = CallingConvention.Cdecl)]
    public static extern int Close(IntPtr db);
    [DllImport("winsqlite3.dll", EntryPoint = "sqlite3_exec", CallingConvention = CallingConvention.Cdecl)]
    public static extern int Exec(IntPtr db, string sql, IntPtr callback, IntPtr errmsgArg, out IntPtr errmsg);
}
"@

try { Add-Type -TypeDefinition $typeDefinition -ErrorAction SilentlyContinue } catch { }

function Resolve-LocalPath($PathValue) {
    $expanded = [System.Environment]::ExpandEnvironmentVariables($PathValue)
    if ([System.IO.Path]::IsPathRooted($expanded)) { return $expanded }
    return [System.IO.Path]::GetFullPath((Join-Path (Get-Location) $expanded))
}

function Escape-SqlVal($val) {
    if ($null -eq $val) { return "NULL" }
    if ($val -is [bool]) { return [int]$val }
    if ($val -is [int] -or $val -is [double] -or $val -is [long]) { return $val }
    $str = $val.ToString().Replace("'", "''")
    return "'$str'"
}

$resolvedDbPath = Resolve-LocalPath $DbPath
if (-not (Test-Path $resolvedDbPath)) {
    Write-Error "Database not found at: $resolvedDbPath"
    exit 1
}

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$inputFile = if (Test-Path $JsonPath) { Resolve-LocalPath $JsonPath } else { Join-Path $scriptDir "accounts.json" }
if (-not (Test-Path $inputFile)) {
    Write-Error "accounts.json not found."
    exit 1
}

$accountsRaw = Get-Content -Raw -Path $inputFile | ConvertFrom-Json
if ($accountsRaw -is [System.Array]) {
    # Handle accidental nested array shape from legacy PowerShell JSON handling.
    if ($accountsRaw.Count -eq 1 -and $accountsRaw[0] -is [System.Array]) {
        $accounts = @($accountsRaw[0])
    } else {
        $accounts = @($accountsRaw)
    }
} else {
    $accounts = @($accountsRaw)
}
$db = [IntPtr]::Zero
if ([WinSQLite3]::Open($resolvedDbPath, [ref]$db) -ne 0) {
    Write-Error "Failed to open database."
    exit 1
}

$importedCount = 0
try {
    foreach ($acc in $accounts) {
        if (-not $acc.accessToken) { continue }
        $id = if ($acc.id) { $acc.id } else { [Guid]::NewGuid().ToString() }
        $email = if ($acc.email) { $acc.email } else { "imported-$importedCount@example.com" }
        $name = if ($acc.name) { $acc.name } else { $email }
        $provider = if ($acc.provider) { $acc.provider } else { "codex" }
        $authType = if ($acc.authType) { $acc.authType } else { "oauth" }
        $priority = if ($null -ne $acc.priority) { $acc.priority } else { 9 }
        $isActive = if ($null -ne $acc.isActive) { $acc.isActive } else { $true }
        $now = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ss.fffZ")
        $createdAt = if ($acc.createdAt) { $acc.createdAt } else { $now }
        $updatedAt = if ($acc.updatedAt) { $acc.updatedAt } else { $now }

        $dataObj = @{
            accessToken = $acc.accessToken
            testStatus = if ($acc.testStatus) { $acc.testStatus } else { "active" }
        }
        if ($acc.refreshToken) { $dataObj.refreshToken = $acc.refreshToken }
        if ($acc.expiresAt) { $dataObj.expiresAt = $acc.expiresAt }
        if ($acc.expiresIn) { $dataObj.expiresIn = $acc.expiresIn }
        if ($acc.providerSpecificData) { $dataObj.providerSpecificData = $acc.providerSpecificData }
        $dataJson = ConvertTo-Json $dataObj -Compress -Depth 10

        $deleteSql = "DELETE FROM providerConnections WHERE provider = 'codex' AND lower(email) = lower($(Escape-SqlVal $email)) AND id <> $(Escape-SqlVal $id);"
        $insertSql = "INSERT OR REPLACE INTO providerConnections (id, provider, authType, name, email, priority, isActive, data, createdAt, updatedAt) VALUES ($(Escape-SqlVal $id), $(Escape-SqlVal $provider), $(Escape-SqlVal $authType), $(Escape-SqlVal $name), $(Escape-SqlVal $email), $(Escape-SqlVal $priority), $(Escape-SqlVal $isActive), $(Escape-SqlVal $dataJson), $(Escape-SqlVal $createdAt), $(Escape-SqlVal $updatedAt));"
        $errmsg = [IntPtr]::Zero
        if ([WinSQLite3]::Exec($db, $deleteSql + $insertSql, [IntPtr]::Zero, [IntPtr]::Zero, [ref]$errmsg) -eq 0) {
            Write-Host "Imported: $email" -ForegroundColor Green
            $importedCount++
        } else {
            Write-Host "Failed: $email" -ForegroundColor Red
        }
    }
} finally {
    [WinSQLite3]::Close($db) | Out-Null
}

Write-Host ""
Write-Host "Done! Imported/updated $importedCount accounts." -ForegroundColor Green
"#
}

fn windows_run_bat() -> &'static str {
    r#"@echo off
title 9Router Account Importer
echo ==============================================
echo       9Router Account Importer - Windows
echo ==============================================
echo.
echo Close 9Router before running this script.
echo.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0Import-Windows.ps1"
echo.
pause
"#
}

fn unix_import_py() -> &'static str {
    r#"#!/usr/bin/env python3
import json
import os
import platform
import sqlite3
import sys
import uuid
from datetime import datetime, timezone

def default_db_path():
    override = os.environ.get("NINEROUTER_DB") or os.environ.get("ROUTER9_DB")
    if override:
        return os.path.expanduser(override)
    if platform.system().lower() == "darwin":
        return os.path.expanduser("~/Library/Application Support/9router/db/data.sqlite")
    return os.path.expanduser("~/.9router/db/data.sqlite")

def now_iso():
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds").replace("+00:00", "Z")

def main():
    script_dir = os.path.dirname(os.path.abspath(__file__))
    json_path = sys.argv[1] if len(sys.argv) > 1 else os.path.join(script_dir, "accounts.json")
    db_path = sys.argv[2] if len(sys.argv) > 2 else default_db_path()
    json_path = os.path.expanduser(json_path)
    db_path = os.path.expanduser(db_path)

    print("==============================================")
    print("      9Router Account Importer - Unix")
    print("==============================================")
    print("Close 9Router before running this script.")
    print("")

    if not os.path.exists(json_path):
        print(f"Error: accounts JSON not found: {json_path}")
        sys.exit(1)
    if not os.path.exists(db_path):
        print(f"Error: 9Router database not found: {db_path}")
        sys.exit(1)

    with open(json_path, "r", encoding="utf-8") as handle:
        accounts = json.load(handle)
    if isinstance(accounts, dict):
        accounts = [accounts]

    conn = sqlite3.connect(db_path, timeout=30)
    imported = 0
    try:
        for acc in accounts:
            if not acc.get("accessToken"):
                continue
            acc_id = acc.get("id") or str(uuid.uuid4())
            email = acc.get("email") or f"imported-{imported}@example.com"
            name = acc.get("name") or email
            provider = acc.get("provider") or "codex"
            auth_type = acc.get("authType") or "oauth"
            priority = acc.get("priority", 9)
            is_active = 1 if acc.get("isActive", True) else 0
            created_at = acc.get("createdAt") or now_iso()
            updated_at = acc.get("updatedAt") or now_iso()

            data_obj = {
                "accessToken": acc.get("accessToken"),
                "testStatus": acc.get("testStatus") or "active",
            }
            for key in ("refreshToken", "expiresAt", "expiresIn"):
                if acc.get(key) is not None:
                    data_obj[key] = acc.get(key)
            if acc.get("providerSpecificData") is not None:
                data_obj["providerSpecificData"] = acc.get("providerSpecificData")

            conn.execute(
                "DELETE FROM providerConnections WHERE provider = 'codex' AND lower(email) = lower(?) AND id <> ?",
                (email, acc_id),
            )
            conn.execute(
                """
                INSERT OR REPLACE INTO providerConnections
                (id, provider, authType, name, email, priority, isActive, data, createdAt, updatedAt)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    acc_id,
                    provider,
                    auth_type,
                    name,
                    email,
                    priority,
                    is_active,
                    json.dumps(data_obj, separators=(",", ":")),
                    created_at,
                    updated_at,
                ),
            )
            print(f"Imported: {email}")
            imported += 1
        conn.commit()
    finally:
        conn.close()

    print("")
    print(f"Done! Imported/updated {imported} accounts.")

if __name__ == "__main__":
    main()
"#
}

fn unix_run_sh(platform_name: &str) -> String {
    let db_path = if platform_name == "macOS" {
        "$HOME/Library/Application Support/9router/db/data.sqlite"
    } else {
        "$HOME/.9router/db/data.sqlite"
    };
    format!(
        r#"#!/bin/bash
set -e
DIR="$( cd "$( dirname "${{BASH_SOURCE[0]}}" )" && pwd )"
echo "=============================================="
echo "      9Router Account Importer - {platform_name}"
echo "=============================================="
echo "Close 9Router before running this script."
echo ""
python3 "$DIR/import-unix.py" "$DIR/accounts.json" "{db_path}"
echo ""
read -p "Press Enter to exit..."
"#
    )
}

fn export_readme() -> &'static str {
    r#"9Router Import Export

This folder contains accounts.json plus import scripts for all 3 platforms:

- Windows: Run-Import-Windows.bat
- macOS: Run-Import-macOS.sh
- Linux: Run-Import-Linux.sh

Before importing:
1. Close 9Router.
2. Run the script for your platform.
3. Reopen 9Router and refresh Provider Limits / Quota Tracker.

The scripts import into providerConnections with provider=codex and authType=oauth.
"#
}

fn zip_text_file(
    zip: &mut zip::ZipWriter<File>,
    file_name: &str,
    contents: &str,
    mode: u32,
) -> Result<(), String> {
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(mode);
    zip.start_file(file_name, options)
        .map_err(|e| e.to_string())?;
    zip.write_all(contents.as_bytes())
        .map_err(|e| e.to_string())
}

fn create_9router_export_zip(
    zip_path: &Path,
    accounts_json: &str,
    selected_emails: &[String],
) -> Result<(), String> {
    let zip_file = File::create(zip_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(zip_file);

    zip_text_file(&mut zip, "accounts.json", accounts_json, 0o644)?;
    zip_text_file(&mut zip, "Import-Windows.ps1", windows_import_ps1(), 0o644)?;
    zip_text_file(&mut zip, "Run-Import-Windows.bat", windows_run_bat(), 0o644)?;
    zip_text_file(&mut zip, "import-unix.py", unix_import_py(), 0o755)?;
    zip_text_file(
        &mut zip,
        "Run-Import-macOS.sh",
        &unix_run_sh("macOS"),
        0o755,
    )?;
    zip_text_file(
        &mut zip,
        "Run-Import-Linux.sh",
        &unix_run_sh("Linux"),
        0o755,
    )?;
    zip_text_file(&mut zip, "README.txt", export_readme(), 0o644)?;
    zip_text_file(
        &mut zip,
        "selected_accounts.txt",
        &selected_emails.join("\n"),
        0o644,
    )?;

    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

fn sanitize_zip_name_part(value: &str) -> String {
    let mut out = String::new();
    let source = value.split('@').next().unwrap_or(value);
    for ch in source.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
        if out.len() >= 48 {
            break;
        }
    }
    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "selected".to_string()
    } else {
        trimmed.to_string()
    }
}

fn make_export_zip_filename(emails: &[String], exported: usize, timestamp: &str) -> String {
    let first = emails
        .first()
        .map(|email| sanitize_zip_name_part(email))
        .unwrap_or_else(|| "selected".to_string());
    let subject = if exported > 1 {
        format!("{}_and_{}_more", first, exported - 1)
    } else {
        first
    };
    format!("9router_{}acc_{}_{}.zip", exported, subject, timestamp)
}

#[tauri::command]
pub fn export_selected_9router_scripts(
    emails: Vec<String>,
) -> Result<NineRouterScriptExportResult, String> {
    let requested_emails: Vec<String> = emails
        .into_iter()
        .map(|email| email.trim().to_lowercase())
        .filter(|email| !email.is_empty())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    if requested_emails.is_empty() {
        return Err("Chưa chọn tài khoản nào để export script 9Router.".to_string());
    }

    let token_map = read_token_map();
    let account_id_map = read_account_id_map();
    let mut sorted_emails = requested_emails;
    sorted_emails.sort();

    let mut accounts = Vec::new();
    let mut skipped_no_token = 0usize;
    let mut skipped_invalid_token = 0usize;

    for email in &sorted_emails {
        let token = match token_map.get(email) {
            Some(token) => token.trim(),
            None => {
                skipped_no_token += 1;
                continue;
            }
        };

        if !is_chatgpt_access_token(token) {
            skipped_invalid_token += 1;
            continue;
        }

        let account_id = account_id_map.get(email).map(|value| value.as_str());
        accounts.push(make_9router_export_account(email, token, account_id)?);
    }

    if accounts.is_empty() {
        return Err(
            "Không export được acc nào vì các acc đã chọn chưa có access token hợp lệ.".to_string(),
        );
    }

    let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let export_dir = Path::new("data/results/9router_exports");
    fs::create_dir_all(export_dir).map_err(|e| e.to_string())?;

    let accounts_json = serde_json::to_string_pretty(&accounts).map_err(|e| e.to_string())?;
    let zip_filename = make_export_zip_filename(&sorted_emails, accounts.len(), &timestamp);
    let zip_path = export_dir.join(zip_filename);
    create_9router_export_zip(&zip_path, &accounts_json, &sorted_emails)?;

    let absolute_export_dir = fs::canonicalize(&export_dir)
        .unwrap_or_else(|_| export_dir.to_path_buf())
        .display()
        .to_string();
    let absolute_zip_path = fs::canonicalize(&zip_path)
        .unwrap_or_else(|_| zip_path.clone())
        .display()
        .to_string();

    Ok(NineRouterScriptExportResult {
        export_dir: absolute_export_dir,
        accounts_json: String::new(),
        zip_path: absolute_zip_path,
        requested: sorted_emails.len(),
        exported: accounts.len(),
        skipped_no_token,
        skipped_invalid_token,
    })
}

#[tauri::command]
pub fn import_plus_real_to_9router() -> Result<NineRouterImportResult, String> {
    let plus_real_emails: Vec<String> = read_lines_safe(PLUS_VERIFIED_REAL_FILE)
        .into_iter()
        .map(|email| email.trim().to_lowercase())
        .filter(|email| !email.is_empty())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    let plus_real_count = plus_real_emails.len();
    if plus_real_count == 0 {
        return Err(
            "Chưa có acc Plus Trial Thật trong data/results/05_plus_verified_real.txt.".to_string(),
        );
    }

    let token_map = read_token_map();
    let token_count = token_map
        .values()
        .filter(|token| is_chatgpt_access_token(token))
        .count();
    let account_id_map = read_account_id_map();

    let db_path = default_9router_db_path()?;
    if !db_path.exists() {
        return Err(format!(
            "Không tìm thấy database 9Router tại {}. Mở 9Router một lần trước rồi thử lại.",
            db_path.display()
        ));
    }

    let mut conn =
        Connection::open(&db_path).map_err(|e| format!("Không mở được DB 9Router: {}", e))?;
    let has_table: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='providerConnections'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if has_table == 0 {
        return Err("DB 9Router thiếu bảng providerConnections.".to_string());
    }

    let transaction = conn.transaction().map_err(|e| e.to_string())?;
    let mut imported = 0usize;
    let mut updated = 0usize;
    let mut skipped_no_token = 0usize;
    let mut skipped_invalid_token = 0usize;

    let mut sorted_emails = plus_real_emails;
    sorted_emails.sort();

    for email in sorted_emails {
        let token = match token_map.get(&email) {
            Some(token) => token.trim(),
            None => {
                skipped_no_token += 1;
                continue;
            }
        };

        if !is_chatgpt_access_token(token) {
            skipped_invalid_token += 1;
            continue;
        }

        let account_id = account_id_map.get(&email).map(|value| value.as_str());
        let token_data = make_9router_token_data(token, &email, account_id)?;
        let now = now_iso_utc();

        let existing_id: Option<String> = transaction
            .query_row(
                "SELECT id FROM providerConnections WHERE provider = 'codex' AND lower(email) = lower(?1) LIMIT 1",
                params![email],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .or_else(|| {
                transaction
                    .query_row(
                        "SELECT id FROM providerConnections WHERE provider = 'codex' AND id = ?1 LIMIT 1",
                        params![token_data.account_id],
                        |row| row.get(0),
                    )
                    .optional()
                    .ok()
                    .flatten()
            });

        if let Some(existing_id) = existing_id {
            let target_id_exists: i64 = transaction
                .query_row(
                    "SELECT COUNT(*) FROM providerConnections WHERE id = ?1 AND id <> ?2",
                    params![token_data.account_id, existing_id],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            let new_id = if target_id_exists == 0 {
                token_data.account_id.clone()
            } else {
                existing_id.clone()
            };

            transaction
                .execute(
                    "UPDATE providerConnections
                     SET id = ?1,
                         authType = 'oauth',
                         name = ?2,
                         email = ?3,
                         priority = COALESCE(priority, 9),
                         isActive = 1,
                         data = ?4,
                         updatedAt = ?5
                     WHERE id = ?6 AND provider = 'codex'",
                    params![new_id, email, email, token_data.data_json, now, existing_id],
                )
                .map_err(|e| e.to_string())?;
            updated += 1;
        } else {
            let id = unique_connection_id(&transaction, &token_data.account_id)?;
            transaction
                .execute(
                    "INSERT INTO providerConnections
                     (id, provider, authType, name, email, priority, isActive, data, createdAt, updatedAt)
                     VALUES (?1, 'codex', 'oauth', ?2, ?3, 9, 1, ?4, ?5, ?6)",
                    params![id, email, email, token_data.data_json, now, now],
                )
                .map_err(|e| e.to_string())?;
            imported += 1;
        }
    }

    transaction.commit().map_err(|e| e.to_string())?;

    Ok(NineRouterImportResult {
        db_path: db_path.display().to_string(),
        plus_real_count,
        token_count,
        imported,
        updated,
        skipped_no_token,
        skipped_invalid_token,
    })
}
