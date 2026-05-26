#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const DEFAULT_ACCOUNT_FILE = path.resolve("src-tauri/data/accounts_list.txt");
const TOKEN_URL = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const GRAPH_BASE = "https://graph.microsoft.com/v1.0";

const emailArg = process.argv[2];
const accountFile = process.env.ACCOUNTS_FILE || DEFAULT_ACCOUNT_FILE;

if (!emailArg) {
  console.error("Usage: node scripts/test-ms-graph-otp.mjs <email>");
  process.exit(1);
}

function parseAccountLine(line) {
  const parts = line.split("|").map((part) => part.trim());
  return {
    email: parts[0] || "",
    password: parts[1] || "",
    refreshToken: parts[2] || "",
    clientId: parts[3] || "",
  };
}

function readAccount(email) {
  const lines = fs.readFileSync(accountFile, "utf8").split(/\r?\n/);
  const line = lines.find((item) => item.trim().toLowerCase().startsWith(`${email.toLowerCase()}|`));
  if (!line) {
    throw new Error(`Không tìm thấy account trong ${accountFile}`);
  }

  const account = parseAccountLine(line);
  if (!account.refreshToken || !account.clientId) {
    throw new Error("Account thiếu refresh_token hoặc client_id");
  }
  return account;
}

async function exchangeToken(account) {
  const body = new URLSearchParams({
    client_id: account.clientId,
    grant_type: "refresh_token",
    refresh_token: account.refreshToken,
    scope: "offline_access https://graph.microsoft.com/Mail.Read",
  });

  const response = await fetch(TOKEN_URL, {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body,
  });

  const text = await response.text();
  let json;
  try {
    json = JSON.parse(text);
  } catch {
    throw new Error(`Token response không phải JSON: HTTP ${response.status}`);
  }

  if (!response.ok || !json.access_token) {
    const detail = json.error_description || json.error || `HTTP ${response.status}`;
    throw new Error(`Không đổi được Microsoft access token: ${detail}`);
  }

  return json.access_token;
}

async function graphGet(accessToken, endpoint) {
  const response = await fetch(`${GRAPH_BASE}${endpoint}`, {
    headers: {
      authorization: `Bearer ${accessToken}`,
      accept: "application/json",
    },
  });

  const text = await response.text();
  let json;
  try {
    json = JSON.parse(text);
  } catch {
    throw new Error(`Graph response không phải JSON: HTTP ${response.status}`);
  }

  if (!response.ok) {
    const detail = json.error?.message || `HTTP ${response.status}`;
    throw new Error(`Graph lỗi ở ${endpoint}: ${detail}`);
  }

  return json;
}

function extractOtp(text) {
  const match = String(text || "").match(/(?<!\d)\d{6}(?!\d)/);
  return match?.[0] || "";
}

function normalizeMessage(item, source) {
  const text = [
    item.subject,
    item.bodyPreview,
    item.from?.emailAddress?.address,
    item.from?.emailAddress?.name,
  ].join("\n");

  return {
    source,
    received: item.receivedDateTime || "",
    from: item.from?.emailAddress?.address || item.from?.emailAddress?.name || "",
    subject: item.subject || "",
    otp: extractOtp(text),
    preview: String(item.bodyPreview || "").replace(/\s+/g, " ").slice(0, 140),
  };
}

async function fetchMessages(accessToken) {
  const select = "$select=subject,from,receivedDateTime,bodyPreview";
  const order = "$orderby=receivedDateTime desc";
  const top = "$top=10";
  const endpoints = [
    `/me/messages?${select}&${order}&${top}`,
    `/me/mailFolders/inbox/messages?${select}&${order}&${top}`,
    `/me/mailFolders/junkemail/messages?${select}&${order}&${top}`,
  ];

  const results = [];
  for (const endpoint of endpoints) {
    const json = await graphGet(accessToken, endpoint);
    const source = endpoint.includes("junkemail")
      ? "junk"
      : endpoint.includes("inbox")
        ? "inbox"
        : "all";
    for (const item of json.value || []) {
      results.push(normalizeMessage(item, source));
    }
  }

  const seen = new Set();
  return results.filter((item) => {
    const key = `${item.received}|${item.from}|${item.subject}|${item.preview}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

const account = readAccount(emailArg);
console.log(`Testing Microsoft Graph for ${account.email}`);
console.log(`Account file: ${accountFile}`);

const accessToken = await exchangeToken(account);
console.log("Token exchange: OK");

const messages = await fetchMessages(accessToken);
console.log(`Messages fetched: ${messages.length}`);

for (const msg of messages.slice(0, 15)) {
  const otpText = msg.otp ? ` OTP=${msg.otp}` : "";
  console.log(`[${msg.source}] ${msg.received} | ${msg.from} | ${msg.subject}${otpText}`);
  if (msg.preview) {
    console.log(`  ${msg.preview}`);
  }
}

const latestOtp = messages.find((msg) => msg.otp);
console.log(latestOtp ? `Latest 6-digit OTP found: ${latestOtp.otp}` : "No 6-digit OTP found.");
