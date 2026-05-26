import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { message } from "antd";
import {
  STATUS_TAB_LABELS,
  WORKFLOW_STEPS,
  buildUnifiedAccountsList,
  getLogColor,
  getPlusMailBadge,
  getProgressSortScore,
  isChatGptAccessToken,
  normalizePlusMailResult,
} from "../lib/accountConsole";

export function useConsoleState(navItems) {
  const [navTab, setNavTab] = useState("accounts");
  const [accounts, setAccounts] = useState([]);
  const [phone, setPhone] = useState("");
  const [autoPipeline, setAutoPipeline] = useState(() => localStorage.getItem("autoPipeline") === "true");
  const [runMode, setRunMode] = useState(() => localStorage.getItem("runMode") || "parallel");
  const [threadCount, setThreadCount] = useState(() => {
    const saved = Number(localStorage.getItem("threadCount"));
    return Number.isFinite(saved) && saved > 0 ? saved : 3;
  });
  const [logs, setLogs] = useState([]);
  const [status, setStatus] = useState("ready");
  const [activeWorkflow] = useState("auth");
  const [runningEmails, setRunningEmails] = useState([]);
  const [isScanningPlusMail, setIsScanningPlusMail] = useState(false);
  const [isImporting9Router, setIsImporting9Router] = useState(false);
  const [isExporting9Router, setIsExporting9Router] = useState(false);
  const [last9RouterExportDir, setLast9RouterExportDir] = useState("");

  const [activeStatusTab, setActiveStatusTab] = useState("Pending");
  const [subFilter, setSubFilter] = useState("All");
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedEmails, setSelectedEmails] = useState([]);

  const [rawFiles, setRawFiles] = useState({
    accounts_list: "",
    access_tokens: "",
    stripe_links: "",
    paypal_links: "",
    profile_run_ips: "",
  });

  const [showImportModal, setShowImportModal] = useState(false);
  const [bulkText, setBulkText] = useState("");

  const terminalEndRef = useRef(null);
  const lastLogRef = useRef({ text: "", level: "", at: 0 });

  function addLog(text, level = "info", options = {}) {
    const nowMs = Date.now();
    const dedupeWindowMs = options.dedupeWindowMs ?? 1200;
    const lastLog = lastLogRef.current;

    if (
      !options.allowDuplicate &&
      lastLog.text === text &&
      lastLog.level === level &&
      nowMs - lastLog.at < dedupeWindowMs
    ) {
      return;
    }

    lastLogRef.current = { text, level, at: nowMs };
    const now = new Date(nowMs);
    const timeStr = `${now.getHours().toString().padStart(2, "0")}:${now
      .getMinutes()
      .toString()
      .padStart(2, "0")}:${now.getSeconds().toString().padStart(2, "0")}`;
    setLogs((prev) => [...prev, { text, level, time: timeStr }]);
  }

  const loadData = async (options = {}) => {
    try {
      const data = await invoke("get_initial_data");
      setPhone(data.phone);
      setRawFiles({
        accounts_list: data.accounts_list,
        access_tokens: data.access_tokens,
        stripe_links: data.stripe_links,
        paypal_links: data.paypal_links,
        profile_run_ips: data.profile_run_ips,
      });
      setAccounts(buildUnifiedAccountsList(data));
      if (!options.silent) {
        addLog("Hệ thống nạp cấu hình và đồng bộ dữ liệu tài khoản thành công!", "info", {
          dedupeWindowMs: 5000,
        });
      }
    } catch (err) {
      addLog(`Lỗi nạp cấu hình ban đầu: ${err}`, "error");
      message.error({ content: `Lỗi tải dữ liệu: ${err}`, duration: 4 });
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  useEffect(() => {
    terminalEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [logs]);

  useEffect(() => {
    localStorage.setItem("runMode", runMode);
  }, [runMode]);

  useEffect(() => {
    localStorage.setItem("threadCount", String(threadCount));
  }, [threadCount]);

  useEffect(() => {
    let unlistenLog;
    let unlistenStatus;

    async function setupListeners() {
      unlistenLog = await listen("automation-log", (event) => {
        const logMsg = event.payload;
        addLog(logMsg);

        if (logMsg.includes("THÀNH CÔNG") || logMsg.includes("HOÀN TẤT") || logMsg.includes("IP đang chạy profile")) {
          loadData({ silent: true });
        }
      });

      unlistenStatus = await listen("automation-status", (event) => {
        const newStatus = event.payload;
        if (newStatus === "idle") {
          setStatus("ready");
          setRunningEmails([]);
          loadData({ silent: true });
        }
      });
    }

    setupListeners();

    return () => {
      if (unlistenLog) unlistenLog();
      if (unlistenStatus) unlistenStatus();
    };
  }, []);

  const getNextSmartWorkflow = (acc) => {
    if (!acc.token) return "auth";
    if (!acc.checkoutUrl) return "make_payment";
    if (!acc.paypalLink) return "confirm_paypal";
    if (acc.status !== "Success") return "paypal_approve";
    return "auth";
  };

  const inferBatchWorkflow = (targetAccounts) => {
    const workflowPriority = ["paypal_approve", "confirm_paypal", "make_payment", "auth"];
    const counts = new Map();

    targetAccounts.forEach((acc) => {
      const step = getNextSmartWorkflow(acc);
      counts.set(step, (counts.get(step) || 0) + 1);
    });

    return Array.from(counts.entries())
      .sort((a, b) => {
        if (b[1] !== a[1]) return b[1] - a[1];
        return workflowPriority.indexOf(a[0]) - workflowPriority.indexOf(b[0]);
      })[0]?.[0] || activeWorkflow;
  };

  const countStats = (statusName) => accounts.filter((account) => account.status === statusName).length;

  const countSubFilter = (type) => {
    if (type === "Login OK") return accounts.filter((account) => account.token !== "").length;
    if (type === "No Token") return accounts.filter((account) => account.token === "").length;
    return accounts.length;
  };

  const filteredAccounts = accounts
    .filter((acc) => {
      let matchesTab = false;
      if (activeStatusTab === "Pending") matchesTab = acc.status === "Pending";
      else if (activeStatusTab === "Fail") matchesTab = acc.status === "Fail";
      else if (activeStatusTab === "Success") matchesTab = acc.status === "Success";
      else if (activeStatusTab === "Sold") matchesTab = acc.status === "Sold";

      let matchesSub = true;
      if (subFilter === "Login OK") matchesSub = acc.token !== "";
      else if (subFilter === "No Token") matchesSub = acc.token === "";

      const matchesSearch = acc.email.toLowerCase().includes(searchQuery.toLowerCase());
      return matchesTab && matchesSub && matchesSearch;
    })
    .sort((a, b) => {
      const scoreDiff = getProgressSortScore(b) - getProgressSortScore(a);
      if (scoreDiff !== 0) return scoreDiff;
      return a.email.localeCompare(b.email);
    });

  const handleStartAutomation = async (email = null, customMode = null) => {
    if (status === "running") return;
    setLogs([]);

    const emailsToRun = email ? [email] : selectedEmails;
    const backendEmails = emailsToRun.length > 0 ? emailsToRun : accounts.map((account) => account.email);
    const targetAccounts = email
      ? accounts.filter((acc) => acc.email === email)
      : emailsToRun.length > 0
        ? accounts.filter((acc) => emailsToRun.includes(acc.email))
        : accounts;
    const modeToRun = customMode || (autoPipeline ? "auto_all" : (email ? activeWorkflow : inferBatchWorkflow(targetAccounts)));
    const maxTargets = Math.max(1, targetAccounts.length || backendEmails.length || 1);
    const normalizedThreadCount = Math.max(1, Math.min(Number(threadCount) || 1, maxTargets));
    const requestedThreads = runMode === "sequential" || email
      ? 1
      : normalizedThreadCount;

    setRunningEmails(backendEmails);

    addLog(`🚀 Bắt đầu kích hoạt tiến trình [${modeToRun.toUpperCase()}]...`);
    if (!customMode && !email) {
      addLog(`🧭 Batch tự chọn bước kế tiếp: ${modeToRun.toUpperCase()}`);
    }
    if (emailsToRun.length > 0) {
      addLog(`🎯 Chỉ chạy tự động cho: ${emailsToRun.join(", ")}`);
    } else {
      addLog("🌐 Chạy cho toàn bộ danh sách.");
    }
    addLog(runMode === "sequential"
      ? "🚦 Chế độ chạy: Tuần tự — xử lý xong từng account rồi mới qua account tiếp theo."
      : `🧵 Chế độ chạy: Song song — tối đa ${requestedThreads} account cùng lúc.`
    );

    setStatus("running");

    try {
      await invoke("start_automation", {
        mode: modeToRun,
        emails: backendEmails,
        threads: requestedThreads,
      });
      message.success(`Kích hoạt tiến trình ${modeToRun.toUpperCase()} thành công!`);
    } catch (err) {
      addLog(`Lỗi: ${err}`, "error");
      setStatus("ready");
      setRunningEmails([]);
      message.error({ content: `Lỗi khởi chạy: ${err}`, duration: 4 });
    }
  };

  const handleStopAutomation = async () => {
    addLog("⏹ Yêu cầu dừng toàn bộ các tiến trình...");
    try {
      await invoke("stop_automation");
      addLog("✅ Đã dừng khẩn cấp thành công.");
      setStatus("ready");
      setRunningEmails([]);
      message.warning("Đã dừng toàn bộ luồng tự động hóa!");
    } catch (err) {
      addLog(`Lỗi dừng tiến trình: ${err}`, "error");
      message.error(`Lỗi khi dừng: ${err}`);
    }
  };

  const handleSaveSettings = async () => {
    try {
      await invoke("save_settings", { phone });
      localStorage.setItem("autoPipeline", autoPipeline ? "true" : "false");
      addLog("💾 Cấu hình hệ thống lưu thành công!");
      message.success("Đã lưu cấu hình điện thoại & auto pipeline thành công!");
    } catch (err) {
      addLog(`Lỗi lưu cấu hình: ${err}`, "error");
      message.error(`Không thể lưu cấu hình: ${err}`);
    }
  };

  const handleScanPlusMailStatus = async () => {
    if (isScanningPlusMail) return;

    const emailsToScan = selectedEmails.length > 0 ? selectedEmails : filteredAccounts.map((account) => account.email);
    const tabLabel = STATUS_TAB_LABELS[activeStatusTab] || activeStatusTab;
    if (emailsToScan.length === 0) {
      message.warning("Không có tài khoản nào trong danh sách hiển thị để quét mail!");
      return;
    }

    setIsScanningPlusMail(true);
    addLog(`📬 Bắt đầu quét mail xác nhận Plus Trial ở tab [${tabLabel}] cho ${emailsToScan.length} tài khoản...`);
    message.loading({ content: `Đang quét mail tab ${tabLabel}: ${emailsToScan.length} acc...`, key: "scan_plus_mail", duration: 0 });

    try {
      const rawResults = await invoke("scan_plus_mail_status", { emails: emailsToScan });
      const results = rawResults.map(normalizePlusMailResult);
      const mailSummary = results.reduce((acc, item) => {
        const scanStatus = item.status;
        acc.total += 1;
        if (scanStatus === "Plus Mail Real") acc.plusReal += 1;
        else if (scanStatus === "No Plus Mail") acc.noPlusMail += 1;
        else if (scanStatus === "No Mail Config") acc.noConfig += 1;
        else if (scanStatus === "Mail Server Busy") acc.serverBusy += 1;
        else acc.mailError += 1;
        return acc;
      }, {
        total: 0,
        plusReal: 0,
        noPlusMail: 0,
        noConfig: 0,
        serverBusy: 0,
        mailError: 0,
      });
      const summaryText = `Tab ${tabLabel}: Plus mail thật ${mailSummary.plusReal}/${mailSummary.total} acc | Không có mail Plus ${mailSummary.noPlusMail} | Thiếu config ${mailSummary.noConfig} | Server/API quá tải ${mailSummary.serverBusy} | Lỗi khác ${mailSummary.mailError}`;
      const sampleProblems = results
        .filter((item) => item.status !== "Plus Mail Real")
        .slice(0, 5)
        .map((item) => `${item.email}: ${item.reason || item.status}`);

      addLog(`✨ Hoàn tất quét mail! ${summaryText}`);
      if (sampleProblems.length > 0) {
        addLog(`🧾 Mẫu lỗi quét mail: ${sampleProblems.join(" | ")}`);
      }
      const messageType = mailSummary.serverBusy || mailSummary.mailError ? "warning" : "success";
      message[messageType]({
        content: `Plus mail ${mailSummary.plusReal}/${mailSummary.total}`,
        key: "scan_plus_mail",
        duration: 4,
      });

      setAccounts((prevAccounts) => {
        const resultMap = new Map(results.map((item) => [item.email?.toLowerCase(), item]));
        return prevAccounts.map((acc) => {
          const match = resultMap.get(acc.email.toLowerCase());
          if (!match) return acc;

          const scanStatus = match.status;
          const shouldClearPlusBadge = scanStatus === "No Plus Mail";
          let updatedBadges = acc.badges.filter((badge) => {
            if (badge.startsWith("Mail:")) return false;
            if (shouldClearPlusBadge && badge === "Plus Trial Thật") return false;
            return true;
          });
          if (scanStatus === "Plus Mail Real") {
            updatedBadges = ["Plus Trial Thật", ...updatedBadges.filter((badge) => badge !== "Plus Trial Thật")];
          } else {
            updatedBadges = [getPlusMailBadge(scanStatus), ...updatedBadges];
          }
          return {
            ...acc,
            badges: updatedBadges,
            plusMailStatus: scanStatus,
            plusMailReason: match.reason,
          };
        });
      });
    } catch (err) {
      addLog(`❌ Quét mail thất bại: ${err}`, "error");
      message.error({ content: `Lỗi quét mail: ${err}`, key: "scan_plus_mail" });
    } finally {
      setIsScanningPlusMail(false);
    }
  };

  const handleImportPlusRealTo9Router = async () => {
    if (isImporting9Router) return;

    setIsImporting9Router(true);
    addLog("📤 Đang import toàn bộ acc Plus Trial Thật vào 9Router...");
    message.loading({ content: "Đang import acc Trial Thật vào 9Router...", key: "import_9router", duration: 0 });

    try {
      const result = await invoke("import_plus_real_to_9router");
      const written = result.imported + result.updated;
      addLog(
        `✅ Import 9Router xong: thêm mới ${result.imported}, cập nhật ${result.updated}, bỏ qua không token ${result.skippedNoToken}, token lỗi ${result.skippedInvalidToken}.`
      );
      message.success({ content: `Đã import/cập nhật ${written} acc Trial Thật vào 9Router!`, key: "import_9router", duration: 4 });
      addLog(`📍 9Router DB: ${result.dbPath}`);
    } catch (err) {
      addLog(`❌ Import 9Router thất bại: ${err}`, "error");
      message.error({ content: `Import 9Router lỗi: ${err}`, key: "import_9router", duration: 5 });
    } finally {
      setIsImporting9Router(false);
    }
  };

  const handleOpenFolder = async (folderPath = last9RouterExportDir) => {
    if (!folderPath) {
      message.warning("Chưa có thư mục export nào để mở.");
      return;
    }
    try {
      await invoke("open_folder", { path: folderPath });
    } catch (err) {
      message.error(`Không mở được thư mục: ${err}`);
    }
  };

  const handleExportSelected9RouterScripts = async () => {
    if (isExporting9Router) return;
    if (selectedEmails.length === 0) {
      message.warning("Tick acc cần export script 9Router trước đã.");
      return;
    }

    setIsExporting9Router(true);
    addLog(`📦 Đang export script import 9Router cho ${selectedEmails.length} acc đã chọn...`);
    message.loading({ content: "Đang export bộ script 9Router...", key: "export_9router", duration: 0 });

    try {
      const result = await invoke("export_selected_9router_scripts", { emails: selectedEmails });
      const exportPath = result.zipPath || result.exportDir;
      setLast9RouterExportDir(exportPath);
      try {
        await navigator.clipboard.writeText(exportPath);
      } catch (_) {
        // Clipboard can be blocked by the host OS; the path is still written to logs.
      }

      addLog(
        `✅ Export 9Router xong: ${result.exported}/${result.requested} acc. Bỏ qua không token ${result.skippedNoToken}, token lỗi ${result.skippedInvalidToken}.`
      );
      addLog(`📦 Export path: ${exportPath}`);
      message.success({ content: `Export ${result.exported}/${result.requested} acc. Đã copy path.`, key: "export_9router", duration: 4 });
    } catch (err) {
      addLog(`❌ Export 9Router thất bại: ${err}`, "error");
      message.error({ content: `Export 9Router lỗi: ${err}`, key: "export_9router", duration: 5 });
    } finally {
      setIsExporting9Router(false);
    }
  };

  const triggerGetOTP = async (email, password) => {
    if (!password) {
      message.warning("Tài khoản chưa cấu hình mật khẩu để quét OTP!");
      return;
    }
    addLog(`🔑 Đang kết nối Microsoft Graph để lấy mã OTP cho: ${email}...`);
    try {
      const otp = await invoke("get_otp", { email, pass: password });
      await navigator.clipboard.writeText(otp);
      addLog(`🎉 QUÉT OTP THÀNH CÔNG! Mã: ${otp} (Đã tự động Copy vào Clipboard) ✅`);
      message.success({ content: `OTP ${otp} đã copy`, duration: 4 });
    } catch (err) {
      addLog(`❌ Quét OTP thất bại: ${err}`, "error");
      message.error({ content: `Lỗi quét OTP: ${err}`, duration: 4 });
    }
  };

  const handleCopyToken = async (token, email) => {
    if (!token) {
      message.warning(`Tài khoản ${email} chưa có Access Token!`);
      return;
    }
    try {
      await navigator.clipboard.writeText(token);
      addLog(`🎉 ĐÃ COPY ACCESS TOKEN CỦA ${email} VÀO CLIPBOARD! ✅`);
      message.success("Đã copy Access Token thành công!");
    } catch (err) {
      addLog(`❌ Không thể copy Access Token: ${err}`, "error");
      message.error("Không thể sao chép token!");
    }
  };

  const markMultipleAccountsStatus = async (emails, newStatus, batchName = "", warrantyDays = 3) => {
    if (!emails || emails.length === 0) return;
    const nowMs = Date.now();
    const updated = accounts.map((acc) => {
      if (emails.includes(acc.email)) {
        const updatedAcc = {
          ...acc,
          status: newStatus,
          badges: newStatus === "Success"
            ? (acc.token ? ["PayPal OK (Chờ Quét)", "Login OK"] : ["PayPal OK (Chờ Quét)", "No Token"])
            : (newStatus === "Fail" ? ["Login Failed", "login-failed", "No Token"] : ["Sold"]),
        };
        if (newStatus === "Sold") {
          updatedAcc.soldAt = nowMs;
          updatedAcc.warrantyDays = warrantyDays;
          updatedAcc.batchName = batchName;
        } else {
          delete updatedAcc.soldAt;
          delete updatedAcc.warrantyDays;
          delete updatedAcc.batchName;
        }
        return updatedAcc;
      }
      return acc;
    });

    setAccounts(updated);
    addLog(`Đã chuyển trạng thái ${emails.length} tài khoản thành [${newStatus.toUpperCase()}]`);

    const successList = updated.filter((account) => account.status === "Success").map((account) => account.email);
    const soldList = updated.filter((account) => account.status === "Sold").map((account) => account.email);
    const failList = updated.filter((account) => account.status === "Fail").map((account) => account.email);

    // Rebuild soldDetails dictionary from the updated list
    const soldDetails = {};
    updated.forEach((acc) => {
      if (acc.status === "Sold" && acc.soldAt) {
        soldDetails[acc.email] = {
          soldAt: acc.soldAt,
          warrantyDays: acc.warrantyDays || 3,
          batchName: acc.batchName || "",
        };
      }
    });

    try {
      await invoke("save_file_content", { fileType: "success_emails", content: successList.join("\n") });
      const trialJson = JSON.stringify({
        registered: successList,
        sold: soldList,
        fail: failList,
        soldDetails,
      }, null, 2);
      await invoke("save_file_content", { fileType: "trial_registered", content: trialJson });
      message.success(`Đã cập nhật trạng thái của ${emails.length} acc thành ${newStatus}!`);
    } catch (err) {
      message.error(`Lỗi cập nhật tệp: ${err}`);
    }
  };

  const markAccountStatus = async (email, newStatus, batchName = "", warrantyDays = 3) => {
    await markMultipleAccountsStatus([email], newStatus, batchName, warrantyDays);
  };

  const handleImportBulk = async () => {
    if (!bulkText.trim()) return;

    const lines = bulkText.split("\n");
    const accountsLines = [];
    const accessTokensLines = [];

    lines.forEach((line) => {
      const trimmed = line.trim();
      if (!trimmed) return;

      const parts = trimmed.split("|");
      if (parts.length >= 2) {
        const email = parts[0].trim();
        const password = parts[1].trim();
        const mailRefreshToken = parts[2] ? parts[2].trim() : "";
        const accountId = parts[3] ? parts[3].trim() : "";

        if (mailRefreshToken || accountId) {
          accountsLines.push(`${email}|${password}|${mailRefreshToken}|${accountId}`);
        } else {
          accountsLines.push(`${email}|${password}`);
        }

        if (isChatGptAccessToken(mailRefreshToken)) {
          accessTokensLines.push(`${email}|${mailRefreshToken}`);
        }
      }
    });

    if (accountsLines.length === 0) {
      message.error("Định dạng nhập hàng loạt không hợp lệ! Vui lòng dùng: email|password|token|accountId");
      return;
    }

    try {
      const currentAccounts = rawFiles.accounts_list;
      const updatedAccounts = currentAccounts
        ? `${currentAccounts.trim()}\n${accountsLines.join("\n")}`
        : accountsLines.join("\n");

      await invoke("save_file_content", { fileType: "accounts_list", content: updatedAccounts });

      if (accessTokensLines.length > 0) {
        const currentTokens = rawFiles.access_tokens;
        const updatedTokens = currentTokens
          ? `${currentTokens.trim()}\n${accessTokensLines.join("\n")}`
          : accessTokensLines.join("\n");
        await invoke("save_file_content", { fileType: "access_tokens", content: updatedTokens });
      }

      addLog(`📥 Đã phân tích & nhập hàng loạt thành công ${accountsLines.length} tài khoản!`);
      setShowImportModal(false);
      setBulkText("");
      message.success(`Đã import thành công ${accountsLines.length} tài khoản!`);
      loadData();
    } catch (err) {
      addLog(`Lỗi nhập hàng loạt: ${err}`, "error");
      message.error(`Không thể import hàng loạt: ${err}`);
    }
  };

  const currentNav = navItems.find((item) => item.key === navTab);

  return {
    WORKFLOW_STEPS,
    accounts,
    activeStatusTab,
    autoPipeline,
    bulkText,
    countStats,
    countSubFilter,
    currentNav,
    filteredAccounts,
    getLogColor,
    getNextSmartWorkflow,
    handleCopyToken,
    handleExportSelected9RouterScripts,
    handleImportBulk,
    handleImportPlusRealTo9Router,
    handleOpenFolder,
    handleSaveSettings,
    handleScanPlusMailStatus,
    handleStartAutomation,
    handleStopAutomation,
    isExporting9Router,
    isImporting9Router,
    isScanningPlusMail,
    last9RouterExportDir,
    loadData,
    logs,
    markAccountStatus,
    markMultipleAccountsStatus,
    navTab,
    phone,
    runningEmails,
    runMode,
    searchQuery,
    selectedEmails,
    setActiveStatusTab,
    setAutoPipeline,
    setBulkText,
    setLogs,
    setNavTab,
    setPhone,
    setRunMode,
    setSearchQuery,
    setSelectedEmails,
    setShowImportModal,
    setSubFilter,
    showImportModal,
    status,
    subFilter,
    terminalEndRef,
    threadCount,
    triggerGetOTP,
    setThreadCount,
  };
}
