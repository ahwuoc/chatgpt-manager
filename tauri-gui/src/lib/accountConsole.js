export const WORKFLOW_STEPS = [
  { key: "auto_all", label: "🔥 AUTO TRỌN GÓI (Login ➡️ Tạo Link ➡️ Duyệt PayPal)", shortLabel: "Auto Trọn Gói" },
  { key: "auth", label: "Bước 1: Login OTP", shortLabel: "Login OTP" },
  { key: "make_payment", label: "Bước 2: Stripe Confirm", shortLabel: "Stripe Confirm" },
  { key: "confirm_paypal", label: "Bước 3: Get Paypal URL", shortLabel: "Get Paypal URL" },
  { key: "paypal_approve", label: "Bước 4: Auto Approve PayPal", shortLabel: "Approve PayPal" },
];

export const STATUS_TAB_LABELS = {
  Pending: "Chờ xử lý",
  Fail: "Trial Fail",
  Success: "Trial Success",
  Sold: "Đã bán",
};

export const isChatGptAccessToken = (value) => {
  if (!value) return false;
  const token = value.trim();
  return token.startsWith("eyJ") && token.split(".").length === 3;
};

export const normalizePlusMailResult = (item) => {
  if (Array.isArray(item)) {
    return {
      email: item[0],
      status: item[1],
      reason: item[2] || "",
    };
  }
  return {
    email: item.email,
    status: item.status,
    reason: item.reason || "",
    mailCount: item.mailCount,
    matchedSubject: item.matchedSubject,
    matchedDate: item.matchedDate,
  };
};

export const getPlusMailBadge = (status) => {
  if (status === "No Plus Mail") return "Mail: Không thấy Plus";
  if (status === "No Mail Config") return "Mail: Thiếu config";
  if (status === "Mail Server Busy") return "Mail: Server quá tải";
  if (status === "Mail Error") return "Mail: Lỗi request";
  return `Mail: ${status}`;
};

export const getLogColor = (text, level) => {
  if (level === "error" || text.includes("❌") || text.toLowerCase().includes("lỗi")) {
    return "text-rose-400 font-mono";
  }
  if (text.includes("🎉") || text.includes("✅") || text.includes("THÀNH CÔNG")) {
    return "text-emerald-400 font-semibold font-mono";
  }
  if (text.includes("🚀") || text.includes("🌐")) {
    return "text-sky-300 font-medium font-mono";
  }
  return "text-slate-300 font-mono";
};

export const getProgressSortScore = (acc) => {
  let score = 0;
  if (acc.badges.includes("Plus Trial Thật")) score += 1000;
  if (acc.paypalLink) score += 400;
  if (acc.checkoutUrl) score += 300;
  if (acc.token) score += 200;
  if (acc.badges.includes("Free (Fake/Lỗi)")) score -= 50;
  return score;
};

const uniqueBadges = (badges) => {
  const seen = new Set();
  return badges.filter((badge) => {
    const key = badge.toLowerCase();
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
};

const parseProfileRunIps = (raw) => {
  try {
    const parsed = JSON.parse(raw || "{}");
    if (!parsed || typeof parsed !== "object") return {};
    return parsed;
  } catch (_) {
    return {};
  }
};

export function buildUnifiedAccountsList(data) {
  const map = new Map();
  const profileRunIps = parseProfileRunIps(data.profile_run_ips);

  data.accounts_list.split("\n").forEach((line) => {
    const trimmed = line.trim();
    if (!trimmed) return;
    const parts = trimmed.split("|");
    if (parts.length >= 2) {
      const email = parts[0].trim();
      const password = parts[1].trim();
      const rToken = parts[2] ? parts[2].trim() : "";
      const accountId = parts[3] ? parts[3].trim() : "";
      map.set(email, {
        email,
        password,
        token: "",
        refreshToken: rToken,
        accountId,
        checkoutUrl: "",
        paypalLink: "",
        status: "Pending",
        badges: ["Pending", "not-registered", "No Token"],
        created: "16:45:52 20/05/2026",
        profileRunIp: "",
        profileRunAt: "",
        profileRunFlow: "",
        profileProxyLabel: "",
      });
    }
  });

  data.access_tokens.split("\n").forEach((line) => {
    const trimmed = line.trim();
    if (!trimmed) return;
    const parts = trimmed.split("|");
    if (parts.length >= 2) {
      const email = parts[0].trim();
      const token = parts[1].trim();
      if (!isChatGptAccessToken(token)) return;
      if (map.has(email)) {
        const acc = map.get(email);
        acc.token = token;
        acc.badges = acc.badges.filter((badge) => badge !== "No Token");
        if (!acc.badges.includes("Login OK")) {
          acc.badges.push("Login OK");
        }
      } else {
        map.set(email, {
          email,
          password: "",
          token,
          refreshToken: "",
          accountId: "",
          checkoutUrl: "",
          paypalLink: "",
          status: "Pending",
          badges: ["Login OK"],
          created: "16:45:52 20/05/2026",
          profileRunIp: "",
          profileRunAt: "",
          profileRunFlow: "",
          profileProxyLabel: "",
        });
      }
    }
  });

  data.stripe_links.split("\n").forEach((line) => {
    const trimmed = line.trim();
    if (!trimmed) return;
    let email = "";
    let checkoutUrl = "";
    try {
      const parsed = JSON.parse(trimmed);
      email = parsed.email;
      checkoutUrl = parsed.checkout_url;
    } catch (_) {
      const parts = trimmed.split("|");
      if (parts.length >= 2) {
        email = parts[0].trim();
        checkoutUrl = parts[1].trim();
      }
    }

    if (email && map.has(email)) {
      const acc = map.get(email);
      acc.checkoutUrl = checkoutUrl;
      acc.badges.push("Has Stripe Link");
    }
  });

  data.paypal_links.split("\n").forEach((line) => {
    const trimmed = line.trim();
    if (!trimmed) return;
    const parts = trimmed.split("|");
    if (parts.length >= 2) {
      const email = parts[0].trim();
      const link = parts[1].trim();
      if (map.has(email)) {
        const acc = map.get(email);
        acc.paypalLink = link;
        acc.badges.push("Has PayPal Link");
      }
    }
  });

  data.success_emails.forEach((email) => {
    if (map.has(email)) {
      const acc = map.get(email);
      acc.status = "Success";
      const isVerifiedReal = data.plus_verified_real?.includes(email);
      acc.badges = isVerifiedReal
        ? (acc.token ? ["Plus Trial Thật", "Login OK"] : ["Plus Trial Thật", "No Token"])
        : (acc.token ? ["PayPal OK (Chờ Quét)", "Login OK"] : ["PayPal OK (Chờ Quét)", "No Token"]);
    }
  });

  try {
    const customData = JSON.parse(data.trial_registered);
    if (customData.registered) {
      customData.registered.forEach((email) => {
        if (map.has(email)) {
          const acc = map.get(email);
          acc.status = "Success";
          const isVerifiedReal = data.plus_verified_real?.includes(email);
          acc.badges = isVerifiedReal
            ? (acc.token ? ["Plus Trial Thật", "Login OK"] : ["Plus Trial Thật", "No Token"])
            : (acc.token ? ["PayPal OK (Chờ Quét)", "Login OK"] : ["PayPal OK (Chờ Quét)", "No Token"]);
        }
      });
    }
    if (customData.sold) {
      const soldDetails = customData.soldDetails || {};
      customData.sold.forEach((email) => {
        if (map.has(email)) {
          const acc = map.get(email);
          acc.status = "Sold";
          const details = soldDetails[email] || {};
          acc.soldAt = details.soldAt || null;
          acc.warrantyDays = details.warrantyDays || 3;
          acc.batchName = details.batchName || "";
          const isVerifiedReal = data.plus_verified_real?.includes(email);
          acc.badges = isVerifiedReal
            ? ["Sold", "Plus Trial Thật"]
            : ["Sold", "PayPal OK (Chờ Quét)"];
        }
      });
    }
    if (customData.fail) {
      customData.fail.forEach((email) => {
        if (map.has(email)) {
          const acc = map.get(email);
          acc.status = "Fail";
          const isVerifiedReal = data.plus_verified_real?.includes(email);
          acc.badges = isVerifiedReal
            ? ["Plus Trial Thật", "Login Failed", "No Token"]
            : ["Login Failed", "No Token"];
        }
      });
    }
  } catch (_) {
    // trial_registered may be empty or older format; keep the base account list.
  }

  const list = Array.from(map.values());
  list.forEach((acc) => {
    const ipRecord = profileRunIps[acc.email.toLowerCase()] || profileRunIps[acc.email] || null;
    if (ipRecord) {
      acc.profileRunIp = ipRecord.ip || "";
      acc.profileRunAt = ipRecord.at || "";
      acc.profileRunFlow = ipRecord.flow || "";
      acc.profileProxyLabel = ipRecord.proxyLabel || "";
    }
    if (acc.paypalLink) {
      acc.badges = acc.badges.filter((badge) => badge !== "Has Stripe Link");
    }
    acc.badges = uniqueBadges(acc.badges);
  });

  return list;
}
