import React, { useEffect, useState } from "react";
import { Table, Tag, Input, InputNumber, Button, Segmented, Space, Dropdown, Row, Col, Switch, message, Modal, Select } from "antd";
import { invoke } from "@tauri-apps/api/core";
import {
  SearchOutlined,
  DownloadOutlined,
  SettingOutlined,
  CopyOutlined,
  PlayCircleOutlined,
  EllipsisOutlined,
  LoadingOutlined,
  TeamOutlined,
  WalletOutlined,
  ChromeOutlined,
  GlobalOutlined,
  KeyOutlined,
  CheckCircleOutlined,
  CloseCircleOutlined,
  DollarOutlined,
  ReloadOutlined,
  InboxOutlined,
  CloudUploadOutlined,
} from "@ant-design/icons";

export default function AccountTable({
  accounts,
  filteredAccounts,
  status,
  runningEmails,
  runMode,
  setRunMode,
  threadCount,
  setThreadCount,
  selectedEmails,
  setSelectedEmails,
  activeStatusTab,
  setActiveStatusTab,
  subFilter,
  setSubFilter,
  searchQuery,
  setSearchQuery,
  countStats,
  countSubFilter,
  loadData,
  setShowImportModal,
  getNextSmartWorkflow,
  handleStartAutomation,
  handleCopyToken,
  triggerGetOTP,
  markAccountStatus,
  markMultipleAccountsStatus,
  WORKFLOW_STEPS,
  isScanningPlusMail,
  handleScanPlusMailStatus,
  isImporting9Router,
  handleImportPlusRealTo9Router,
  isExporting9Router,
  handleExportSelected9RouterScripts,
  last9RouterExportDir,
  handleOpenFolder,
}) {
  const selectedAccounts = accounts.filter((acc) => selectedEmails.includes(acc.email));
  const [usProxyStatus, setUsProxyStatus] = useState(null);
  const [checkingUsProxy, setCheckingUsProxy] = useState(false);
  const [changingUsProxy, setChangingUsProxy] = useState(false);
  const [savingUsProxyEnabled, setSavingUsProxyEnabled] = useState(false);

  // States for sold details & batch tracing
  const [soldModalVisible, setSoldModalVisible] = useState(false);
  const [soldTargetEmails, setSoldTargetEmails] = useState([]);
  const [batchName, setBatchName] = useState("");
  const [warrantyDays, setWarrantyDays] = useState(3);
  const [selectedBatchFilter, setSelectedBatchFilter] = useState("All");

  const uniqueBatches = Array.from(new Set(accounts
    .filter((acc) => acc.status === "Sold" && acc.batchName)
    .map((acc) => acc.batchName)
  ));

  const finalFilteredAccounts = filteredAccounts.filter((acc) => {
    if (activeStatusTab === "Sold" && selectedBatchFilter !== "All") {
      return acc.batchName === selectedBatchFilter;
    }
    return true;
  });

  const refreshUsProxyStatus = async (quiet = false) => {
    setCheckingUsProxy(true);
    try {
      const result = await invoke("get_us_browser_proxy_status");
      setUsProxyStatus(result);
      if (!quiet) {
        if (result.currentIp) {
          const countryText = result.countryCode || result.country || "";
          message.success(`Proxy exit IP: ${result.currentIp}${countryText ? ` (${countryText})` : ""}`);
        } else if (result.enabled) {
          message.warning(result.message || "Chưa check được proxy IP.");
        } else {
          message.info(result.message || "Browser proxy đang tắt.");
        }
      }
    } catch (e) {
      setUsProxyStatus((current) => ({
        ...(current || {}),
        enabled: current?.enabled || false,
        currentIp: null,
        message: String(e),
      }));
      if (!quiet) {
        message.error(`Check proxy lỗi: ${e}`);
      }
    } finally {
      setCheckingUsProxy(false);
    }
  };

  const handleChangeUsProxyIp = async () => {
    setChangingUsProxy(true);
    try {
      const result = await invoke("change_us_browser_proxy_ip");
      setUsProxyStatus(result);
      if (result.changed) {
        const countryText = result.countryCode || result.country || "";
        message.success(result.currentIp ? `Đã đổi proxy IP: ${result.currentIp}${countryText ? ` (${countryText})` : ""}` : "Đã gọi đổi proxy IP.");
      } else if (result.waitSeconds) {
        message.warning(`ENODE yêu cầu chờ ${result.waitSeconds}s rồi đổi IP tiếp.`);
      } else {
        message.warning(result.message || "ENODE chưa đổi IP.");
      }
    } catch (e) {
      message.error(`Đổi proxy IP lỗi: ${e}`);
    } finally {
      setChangingUsProxy(false);
    }
  };

  const handleToggleUsProxyEnabled = async (checked) => {
    setSavingUsProxyEnabled(true);
    try {
      const currentConfig = await invoke("get_us_browser_proxy_config");
      await invoke("save_us_browser_proxy_config", {
        config: {
          ...currentConfig,
          enabled: checked,
        },
      });

      setUsProxyStatus((current) => ({
        ...(current || {}),
        enabled: checked,
        message: checked
          ? "Đã bật Browser Proxy. Nhấn Check IP để xác nhận exit IP."
          : "Đã tắt Browser Proxy. Browser sẽ dùng mạng mặc định.",
      }));

      message.success(
        checked
          ? "Đã bật dùng Browser Proxy."
          : "Đã tắt dùng Browser Proxy."
      );

      await refreshUsProxyStatus(true);
    } catch (e) {
      message.error(`Đổi trạng thái proxy lỗi: ${e}`);
      await refreshUsProxyStatus(true);
    } finally {
      setSavingUsProxyEnabled(false);
    }
  };

  useEffect(() => {
    refreshUsProxyStatus(true);
  }, []);

  const activeTabLabel = {
    Pending: "Chờ xử lý",
    Fail: "Trial Fail",
    Success: "Trial Success",
    Sold: "Đã bán",
  }[activeStatusTab] || activeStatusTab;
  const proxyEnabled = usProxyStatus?.enabled ?? false;
  const proxyEndpoint = usProxyStatus?.host
    ? `${usProxyStatus.host}${usProxyStatus.port ? `:${usProxyStatus.port}` : ""}`
    : "Chưa cấu hình";
  const proxyExitIp = usProxyStatus?.currentIp || (proxyEnabled ? "Chưa check" : "Đang tắt");
  const proxyLocation = [
    usProxyStatus?.city,
    usProxyStatus?.region,
    usProxyStatus?.countryCode || usProxyStatus?.country,
  ].filter(Boolean).join(", ");
  const proxyNetwork = usProxyStatus?.isp || usProxyStatus?.asn || "";

  const getVisibleBadges = (acc) => {
    const badges = Array.from(
      new Map(acc.badges.map((badge) => [badge.toLowerCase(), badge])).values()
    ).filter((badge) => !["Pending", "not-registered"].includes(badge));
    const scanBadges = badges.filter((badge) => badge === "Plus Trial Thật" || badge.startsWith("Mail:"));
    const otherBadges = badges.filter((badge) => badge !== "Plus Trial Thật" && !badge.startsWith("Mail:"));
    return [...scanBadges, ...otherBadges].slice(0, 3);
  };

  // ── Table Column Definition ────────────────────────────────────────────────

  const columns = [
    {
      title: "Thông tin Tài khoản",
      key: "account_info",
      render: (_, record) => {
        const nextStep = getNextSmartWorkflow(record);
        const nextStepInfo = WORKFLOW_STEPS.find((step) => step.key === nextStep);
        return (
          <div className="space-y-2.5">
            <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
              <span className="font-semibold text-[15px] tracking-tight text-white">{record.email}</span>
              <Tag color="purple" className="rounded-full px-2.5 py-0.5 text-[10px] font-bold tracking-wider border-none bg-purple-500/10 text-purple-400">
                👉 Bước kế tiếp: {nextStepInfo?.shortLabel || nextStep}
              </Tag>
            </div>
            <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-slate-400">
              <span className="flex items-center gap-1">
                <SettingOutlined style={{ fontSize: 11 }} /> Cấu hình: {record.created}
              </span>
              {record.accountId && (
                <span className="flex items-center gap-1 font-mono text-[11px]">
                  ID: {record.accountId.substring(0, 15)}...
                </span>
              )}
              {record.profileRunIp && (
                <span
                  className="flex items-center gap-1 font-mono text-[11px] text-sky-300"
                  title={[
                    record.profileProxyLabel ? `Proxy: ${record.profileProxyLabel}` : "",
                    record.profileRunFlow ? `Flow: ${record.profileRunFlow}` : "",
                    record.profileRunAt ? `Lúc: ${record.profileRunAt}` : "",
                  ].filter(Boolean).join(" • ")}
                >
                  <GlobalOutlined style={{ fontSize: 11 }} /> IP: {record.profileRunIp}
                </span>
              )}
            </div>
          </div>
        );
      },
    },
    ...(activeStatusTab === "Sold" ? [
      {
        title: "Lô hàng & Bảo hành",
        key: "sold_details",
        width: 250,
        render: (_, record) => {
          if (record.status !== "Sold") return null;
          const soldAtMs = record.soldAt;
          if (!soldAtMs) {
            return <span className="text-slate-500 italic">Chưa cập nhật thông tin</span>;
          }
          const expiryMs = soldAtMs + (record.warrantyDays || 3) * 24 * 60 * 60 * 1000;
          const nowMs = Date.now();
          const diffMs = expiryMs - nowMs;
          
          let countdownText = "";
          let colorClass = "";
          if (diffMs <= 0) {
            countdownText = "❌ Hết bảo hành";
            colorClass = "text-rose-400 font-bold bg-rose-500/10 border border-rose-500/20 px-2.5 py-1 rounded-xl";
          } else {
            const diffDays = Math.floor(diffMs / (24 * 60 * 60 * 1000));
            const diffHours = Math.floor((diffMs % (24 * 60 * 60 * 1000)) / (60 * 60 * 1000));
            countdownText = `🛡️ Còn ${diffDays} ngày ${diffHours} giờ`;
            colorClass = "text-emerald-400 font-bold bg-emerald-500/10 border border-emerald-500/20 px-2.5 py-1 rounded-xl";
          }
          
          return (
            <div className="space-y-1.5 py-1">
              <div className="flex items-center gap-1.5">
                <span className="text-[10px] font-bold uppercase tracking-wider text-slate-500">Lô:</span>
                <span className="text-xs font-semibold text-white bg-slate-800 px-2 py-0.5 rounded-lg border border-slate-700">
                  {record.batchName || "Chưa gán lô"}
                </span>
              </div>
              <div className="flex items-center gap-1.5">
                <span className="text-[10px] font-bold uppercase tracking-wider text-slate-500">BH:</span>
                <span className={`text-[11px] font-mono ${colorClass}`}>
                  {countdownText}
                </span>
              </div>
            </div>
          );
        }
      }
    ] : []),
    {
      title: "Trạng thái / Nhãn",
      key: "badges",
      width: 320,
      render: (_, record) => {
        const visibleBadges = getVisibleBadges(record);
        return (
          <Space size={[4, 8]} wrap>
            {visibleBadges.length === 0 ? (
              <span className="text-xs text-slate-500 italic">Trống</span>
            ) : (
              visibleBadges.map((badge, idx) => {
                if (badge === "Login OK") {
                  return (
                    <Tag
                      key={idx}
                      color="success"
                      className="cursor-pointer rounded-full px-3 py-0.5 text-[10px] font-bold uppercase tracking-wider hover:opacity-85 border-none bg-emerald-500/10 text-emerald-400"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleCopyToken(record.token, record.email);
                      }}
                    >
                      <CopyOutlined className="mr-1" />
                      Access Token
                    </Tag>
                  );
                }
                if (badge === "Has PayPal Link") {
                  return (
                    <Tag
                      key={idx}
                      color="gold"
                      title="Click để copy PayPal link"
                      className="cursor-pointer rounded-full px-3 py-0.5 text-[10px] font-bold uppercase tracking-wider hover:opacity-85 border-none bg-amber-500/10 text-amber-300"
                      onClick={async (e) => {
                        e.stopPropagation();
                        await navigator.clipboard.writeText(record.paypalLink);
                        message.success("Đã copy PayPal link!");
                      }}
                    >
                      <CopyOutlined className="mr-1" />
                      Has PayPal Link
                    </Tag>
                  );
                }
                if (badge === "Has Stripe Link") {
                  return (
                    <Tag
                      key={idx}
                      color="gold"
                      title="Click để copy Stripe checkout link"
                      className="cursor-pointer rounded-full px-3 py-0.5 text-[10px] font-bold uppercase tracking-wider hover:opacity-85 border-none bg-amber-500/10 text-amber-300"
                      onClick={async (e) => {
                        e.stopPropagation();
                        await navigator.clipboard.writeText(record.checkoutUrl);
                        message.success("Đã copy Stripe checkout link!");
                      }}
                    >
                      <CopyOutlined className="mr-1" />
                      Has Stripe Link
                    </Tag>
                  );
                }
                let color = "warning";
                let customClass = "border-none bg-amber-500/10 text-amber-400";
                if (badge === "Reg Trial Plus Success" || badge === "Success") {
                  color = "emerald";
                  customClass = "border-none bg-emerald-500/10 text-emerald-400";
                }
                if (badge === "Plus Trial Thật") {
                  color = "gold";
                  customClass = "border-none bg-yellow-500/15 text-yellow-400 font-extrabold shadow-[0_1px_6px_rgba(234,179,8,0.15)]";
                }
                if (badge === "Free (Fake/Lỗi)") {
                  color = "default";
                  customClass = "border-none bg-slate-800 text-slate-400 line-through";
                }
                if (badge === "Login Failed" || badge === "login-failed") {
                  color = "error";
                  customClass = "border-none bg-rose-500/10 text-rose-400";
                }
                if (badge === "No Token") {
                  color = "default";
                  customClass = "border-none bg-slate-500/10 text-slate-400";
                }
                if (badge === "Sold") {
                  color = "cyan";
                  customClass = "border-none bg-cyan-500/10 text-cyan-400";
                }
                if (badge.startsWith("Mail:")) {
                  color = "default";
                  customClass = "border-none bg-slate-700/70 text-slate-200";
                  if (badge.includes("Không thấy Plus")) {
                    customClass = "border-none bg-slate-500/10 text-slate-300";
                  }
                  if (badge.includes("Thiếu config")) {
                    color = "warning";
                    customClass = "border-none bg-amber-500/10 text-amber-300";
                  }
                  if (badge.includes("Server quá tải")) {
                    color = "orange";
                    customClass = "border-none bg-orange-500/10 text-orange-300";
                  }
                  if (badge.includes("Lỗi request")) {
                    color = "error";
                    customClass = "border-none bg-rose-500/10 text-rose-300";
                  }
                }

                return (
                  <Tag
                    key={idx}
                    color={color}
                    title={badge.startsWith("Mail:") ? record.plusMailReason || badge : undefined}
                    className={`rounded-full px-3 py-0.5 text-[10px] font-bold uppercase tracking-wider ${customClass}`}
                  >
                    {badge}
                  </Tag>
                );
              })
            )}
          </Space>
        );
      },
    },
    {
      title: "Hành động",
      key: "actions",
      width: 200,
      align: "right",
      render: (_, record) => {
        const nextStep = getNextSmartWorkflow(record);
        const isRowRunning = runningEmails.includes(record.email);

        const actionsMenu = {
          items: [
            {
              key: "run_auth",
              label: "Chạy lại Login",
              icon: <KeyOutlined className="text-emerald-400" />,
              onClick: () => handleStartAutomation(record.email, "auth"),
            },
            {
              key: "run_auto_all",
              label: "🔥 Chạy Auto Trọn Gói",
              icon: <PlayCircleOutlined className="text-amber-400 font-extrabold" />,
              onClick: () => handleStartAutomation(record.email, "auto_all"),
            },
            {
              key: "run_smart",
              label: "Chạy bước tiếp theo",
              icon: <PlayCircleOutlined className="text-emerald-400" />,
              onClick: () => handleStartAutomation(record.email, nextStep),
            },
            {
              key: "run_make_payment",
              label: "Tạo lại Link Payment",
              icon: <WalletOutlined className="text-sky-400" />,
              onClick: () => handleStartAutomation(record.email, "make_payment"),
            },
            {
              key: "run_confirm_paypal",
              label: "Lấy lại Link PayPal",
              icon: <WalletOutlined className="text-amber-300" />,
              onClick: () => handleStartAutomation(record.email, "confirm_paypal"),
            },
            {
              key: "run_paypal_approve",
              label: "Chạy lại Duyệt PayPal",
              icon: <ChromeOutlined className="text-amber-400" />,
              onClick: () => handleStartAutomation(record.email, "paypal_approve"),
            },
            {
              key: "open_account_browser",
              label: "Open Browser (acc này)",
              icon: <GlobalOutlined className="text-cyan-300" />,
              onClick: async () => {
                try {
                  await invoke("open_account_browser", { email: record.email });
                  message.success(`Đã mở browser cho ${record.email}`);
                } catch (e) {
                  message.error(`Mở browser lỗi: ${e}`);
                }
              },
            },
            { type: "divider" },
            {
              key: "copy_full",
              label: "Copy Acc Đầy Đủ",
              icon: <TeamOutlined className="text-indigo-400" />,
              onClick: async () => {
                const text = `${record.email} | ${record.password} | ${record.refreshToken || record.token || ""} | ${record.accountId || ""}`;
                await navigator.clipboard.writeText(text);
                message.success(`Đã copy acc ${record.email} dạng đầy đủ!`);
              },
            },
            {
              key: "get_otp",
              label: "Quét OTP Microsoft",
              icon: <KeyOutlined className="text-amber-400" />,
              onClick: () => triggerGetOTP(record.email, record.password),
            },
            { type: "divider" },
            {
              key: "mark_success",
              label: "Đánh dấu Success",
              icon: <CheckCircleOutlined className="text-emerald-400" />,
              onClick: () => markAccountStatus(record.email, "Success"),
            },
            {
              key: "mark_fail",
              label: "Đánh dấu Fail",
              icon: <CloseCircleOutlined className="text-rose-400" />,
              onClick: () => markAccountStatus(record.email, "Fail"),
            },
            {
              key: "mark_sold",
              label: "Đánh dấu Sold",
              icon: <DollarOutlined className="text-cyan-400" />,
              onClick: () => {
                setSoldTargetEmails([record.email]);
                const today = new Date();
                const defaultBatch = `Lô ${today.getDate().toString().padStart(2, '0')}/${(today.getMonth() + 1).toString().padStart(2, '0')}`;
                setBatchName(defaultBatch);
                setWarrantyDays(3);
                setSoldModalVisible(true);
              },
            },
            {
              key: "mark_pending",
              label: "Reset trạng thái (Pending)",
              icon: <ReloadOutlined className="text-slate-400" />,
              onClick: () => markAccountStatus(record.email, "Pending"),
            },
          ],
        };

        return (
          <Space size={8}>
            <Button
              type="primary"
              size="middle"
              className="rounded-xl font-medium min-w-[100px] bg-amber-500 hover:bg-amber-400 border-none text-slate-950"
              disabled={status === "running"}
              onClick={() => handleStartAutomation(record.email, nextStep)}
            >
              {isRowRunning ? (
                <>
                  <LoadingOutlined className="mr-1" />
                  Running
                </>
              ) : (
                <>
                  <PlayCircleOutlined className="mr-1" />
                  Run
                </>
              )}
            </Button>
            <Dropdown menu={actionsMenu} trigger={["click"]}>
              <Button
                variant="outline"
                size="middle"
                className="h-9 w-9 rounded-xl flex items-center justify-center bg-white/5 border-white/10 hover:bg-white/10 hover:border-white/20 text-slate-200"
              >
                <EllipsisOutlined style={{ fontSize: 16 }} />
              </Button>
            </Dropdown>
          </Space>
        );
      },
    },
  ];

  // Expanded details render inside the table
  const expandedRowRender = (record) => {
    return (
      <div className="p-5 border border-white/5 bg-slate-950/45 rounded-2xl space-y-3.5 mx-6 mb-4 select-text">
        <Row gutter={[16, 12]}>
          <Col span={24} md={12}>
            <div className="space-y-1">
              <span className="text-[10px] font-bold uppercase tracking-wider text-slate-500 block">Mật khẩu</span>
              <span className="font-mono text-sm text-slate-200">{record.password || "Chưa cấu hình"}</span>
            </div>
          </Col>
          <Col span={24} md={12}>
            <div className="space-y-1">
              <span className="text-[10px] font-bold uppercase tracking-wider text-slate-500 block">Tài khoản ID (chatgpt_account_id)</span>
              <span className="font-mono text-sm text-slate-200 block truncate">{record.accountId || "Chưa có"}</span>
            </div>
          </Col>
          <Col span={24}>
            <div className="space-y-1">
              <span className="text-[10px] font-bold uppercase tracking-wider text-slate-500 block">Access Token</span>
              {record.token ? (
                <div className="flex items-center gap-3">
                  <span className="font-mono text-xs text-emerald-400 block truncate flex-1 bg-black/30 p-2.5 rounded-xl border border-emerald-950/20">{record.token}</span>
                  <Button
                    size="small"
                    className="flex-shrink-0 rounded-lg"
                    icon={<CopyOutlined />}
                    onClick={() => handleCopyToken(record.token, record.email)}
                  >
                    Copy
                  </Button>
                </div>
              ) : (
                <span className="text-xs text-slate-500 italic">Chưa có Access Token</span>
              )}
            </div>
          </Col>
          {record.checkoutUrl && (
            <Col span={24}>
              <div className="space-y-1">
                <span className="text-[10px] font-bold uppercase tracking-wider text-slate-500 block">Stripe Checkout Link (Bước 2)</span>
                <div className="flex items-center gap-3">
                  <a
                    href={record.checkoutUrl}
                    target="_blank"
                    rel="noreferrer"
                    className="font-mono text-xs text-sky-400 hover:text-sky-300 hover:underline block truncate flex-1 bg-black/30 p-2.5 rounded-xl border border-sky-950/20"
                  >
                    {record.checkoutUrl}
                  </a>
                  <Button
                    size="small"
                    className="flex-shrink-0 rounded-lg"
                    icon={<CopyOutlined />}
                    onClick={async () => {
                      await navigator.clipboard.writeText(record.checkoutUrl);
                      message.success("Đã copy Stripe Checkout Link!");
                    }}
                  >
                    Copy
                  </Button>
                </div>
              </div>
            </Col>
          )}
          {record.paypalLink && (
            <Col span={24}>
              <div className="space-y-1">
                <span className="text-[10px] font-bold uppercase tracking-wider text-slate-500 block">PayPal Approved Link (Bước 3)</span>
                <a
                  href={record.paypalLink}
                  target="_blank"
                  rel="noreferrer"
                  className="font-mono text-xs text-amber-400 hover:text-amber-300 hover:underline block truncate bg-black/30 p-2.5 rounded-xl border border-amber-950/20"
                >
                  {record.paypalLink}
                </a>
              </div>
            </Col>
          )}
        </Row>
      </div>
    );
  };

  const bulkStepMenu = {
    items: [
      {
        key: "bulk_auth",
        label: "Login / lấy Token",
        icon: <KeyOutlined className="text-emerald-400" />,
        onClick: () => handleStartAutomation(null, "auth"),
      },
      {
        key: "bulk_make_payment",
        label: "Tạo Stripe Checkout",
        icon: <WalletOutlined className="text-sky-400" />,
        onClick: () => handleStartAutomation(null, "make_payment"),
      },
      {
        key: "bulk_confirm_paypal",
        label: "Lấy Link PayPal",
        icon: <WalletOutlined className="text-amber-300" />,
        onClick: () => handleStartAutomation(null, "confirm_paypal"),
      },
      {
        key: "bulk_paypal_approve",
        label: "Duyệt PayPal",
        icon: <ChromeOutlined className="text-amber-400" />,
        onClick: () => handleStartAutomation(null, "paypal_approve"),
      },
    ],
  };

  return (
    <div className="flex h-full flex-col gap-5 overflow-hidden p-2">
      {/* Header Filter Panel */}
      <div className="glass rounded-[24px] p-6 border border-white/5 bg-slate-900/40">
        <div className="flex flex-col gap-5">
          {/* Top line Action buttons */}
          <div className="flex flex-col gap-4 xl:flex-row xl:items-center xl:justify-between">
            <div className="space-y-1">
              <div className="flex flex-wrap items-center gap-3">
                <h2 className="text-xl font-bold text-white">Quản lý Tài khoản</h2>
                <Tag color={status === "running" ? "warning" : "success"} className="rounded-full px-2.5 py-0.5 border-none font-semibold">
                  {status === "running" ? "Đang xử lý tự động" : "Sẵn sàng"}
                </Tag>
              </div>
              <p className="text-xs text-slate-400">
                Bộ lọc đang hiển thị: <span className="text-amber-400 font-semibold">{finalFilteredAccounts.length}</span> acc. Đã tick: <span className="text-amber-400 font-semibold">{selectedEmails.length}</span> acc.
              </p>
            </div>

            <Space size={10} wrap>
              <Button type="primary" className="rounded-xl bg-amber-500 hover:bg-amber-400 border-none text-slate-950" onClick={() => setShowImportModal(true)} icon={<DownloadOutlined />}>
                Import Hàng Loạt
              </Button>
              <Button
                type="default"
                className="rounded-xl border-white/10 hover:border-emerald-400 text-slate-200"
                loading={isImporting9Router}
                disabled={status === "running"}
                onClick={handleImportPlusRealTo9Router}
                icon={<CloudUploadOutlined />}
              >
                Import Trial Thật vào 9Router
              </Button>
            </Space>
          </div>

          <div className="flex flex-col gap-3 rounded-2xl border border-sky-500/10 bg-sky-500/5 px-4 py-3 lg:flex-row lg:items-center lg:justify-between">
            <div className="min-w-0">
              <div className="flex flex-wrap items-center gap-2">
                <span className="flex items-center gap-1.5 text-sm font-bold text-sky-200">
                  <ChromeOutlined className="text-sky-400" />
                  Browser Proxy
                </span>
                <Tag
                  color={proxyEnabled ? "processing" : "default"}
                  className="rounded-full border-none px-2.5 py-0.5 text-[10px] font-bold uppercase tracking-wider"
                >
                  {proxyEnabled ? "Bật" : "Tắt"}
                </Tag>
                {proxyEnabled && (usProxyStatus?.countryCode || usProxyStatus?.country) && (
                  <Tag color="blue" className="rounded-full border-none px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider">
                    {usProxyStatus.countryCode || usProxyStatus.country}
                  </Tag>
                )}
              </div>
              <div className="mt-1 flex flex-wrap items-center gap-x-4 gap-y-1 text-[11px] text-slate-400">
                <span className="font-mono">Host: {proxyEndpoint}</span>
                <span className="font-mono text-sky-300">Exit IP: {proxyExitIp}</span>
                {proxyLocation && <span>Vị trí: {proxyLocation}</span>}
                {proxyNetwork && <span className="max-w-[360px] truncate">Mạng: {proxyNetwork}</span>}
                {usProxyStatus?.waitSeconds ? (
                  <span className="text-amber-300">Chờ {usProxyStatus.waitSeconds}s</span>
                ) : (
                  usProxyStatus?.message && (
                    <span className="max-w-[520px] truncate">{usProxyStatus.message}</span>
                  )
                )}
              </div>
            </div>
            <Space size={8} wrap>
              <div className="flex items-center gap-2 rounded-xl border border-white/10 bg-white/5 px-2.5 py-1">
                <span className="text-[11px] font-semibold text-slate-300">Dùng Proxy</span>
                <Switch
                  size="small"
                  checked={proxyEnabled}
                  loading={savingUsProxyEnabled}
                  onChange={handleToggleUsProxyEnabled}
                  activeBg="#38bdf8"
                />
              </div>
              <Button
                size="small"
                icon={<ReloadOutlined />}
                loading={checkingUsProxy}
                onClick={() => refreshUsProxyStatus(false)}
                className="rounded-xl border-white/10 bg-white/5 text-slate-200 hover:border-sky-400"
              >
                Check IP
              </Button>
              <Button
                type="primary"
                size="small"
                icon={<ReloadOutlined />}
                loading={changingUsProxy}
                disabled={!proxyEnabled || !usProxyStatus?.changeIpUrlConfigured}
                onClick={handleChangeUsProxyIp}
                className="rounded-xl border-none bg-sky-500 text-slate-950 hover:bg-sky-400 disabled:bg-slate-700 disabled:text-slate-500"
              >
                Đổi IP
              </Button>
            </Space>
          </div>

          <div className="flex flex-col gap-3 rounded-2xl border border-amber-500/10 bg-amber-500/5 px-4 py-3 lg:flex-row lg:items-center lg:justify-between">
            <div className="min-w-0">
              <div className="flex flex-wrap items-center gap-2">
                <span className="flex items-center gap-1.5 text-sm font-bold text-amber-200">
                  <TeamOutlined className="text-amber-400" />
                  Chế độ chạy account
                </span>
                <Tag
                  color={runMode === "parallel" ? "gold" : "blue"}
                  className="rounded-full border-none px-2.5 py-0.5 text-[10px] font-bold uppercase tracking-wider"
                >
                  {runMode === "parallel" ? `${threadCount} luồng` : "Từng acc"}
                </Tag>
              </div>
              <div className="mt-1 text-[11px] text-slate-400">
                {runMode === "parallel"
                  ? "Chạy nhiều account cùng lúc theo số luồng đã chọn."
                  : "Chạy xong hoàn toàn account hiện tại rồi mới chuyển sang account kế tiếp."}
              </div>
            </div>
            <Space size={8} wrap>
              <Segmented
                value={runMode}
                disabled={status === "running"}
                onChange={setRunMode}
                options={[
                  { label: "Song song", value: "parallel" },
                  { label: "Tuần tự", value: "sequential" },
                ]}
                className="bg-black/25 p-0.5 rounded-[14px]"
              />
              <InputNumber
                min={1}
                max={50}
                value={threadCount}
                disabled={status === "running" || runMode === "sequential"}
                onChange={(value) => setThreadCount(Math.max(1, Number(value) || 1))}
                addonBefore="Luồng"
                className="w-32"
              />
            </Space>
          </div>

          {/* Filter and segmented line */}
          <div className="flex flex-col gap-4 xl:flex-row xl:items-center xl:justify-between">
            <div className="flex flex-wrap items-center gap-2.5">
              <span className="text-xs font-bold uppercase tracking-wider text-slate-400">Trạng thái:</span>
              <Segmented
                value={activeStatusTab}
                onChange={(val) => {
                  setActiveStatusTab(val);
                  setSelectedEmails([]);
                }}
                options={[
                  { label: `Chờ xử lý (${countStats("Pending")})`, value: "Pending" },
                  { label: `Trial Fail (${countStats("Fail")})`, value: "Fail" },
                  { label: `Trial Success (${countStats("Success")})`, value: "Success" },
                  { label: `Đã bán (${countStats("Sold")})`, value: "Sold" },
                ]}
                className="bg-black/25 p-0.5 rounded-[14px]"
              />
              {["Success", "Fail", "Sold"].includes(activeStatusTab) && (
                <Button
                  type="primary"
                  className="bg-gradient-to-r from-emerald-500 to-teal-500 hover:from-emerald-400 hover:to-teal-400 border-none text-slate-950 font-bold rounded-xl h-8 text-[11px] flex items-center gap-1.5 shadow-[0_2px_10px_rgba(16,185,129,0.22)]"
                  loading={isScanningPlusMail}
                  onClick={handleScanPlusMailStatus}
                  icon={<SearchOutlined />}
                >
                  {isScanningPlusMail ? `Đang quét ${activeTabLabel}...` : "Quét Plus qua Hotmail"}
                </Button>
              )}
            </div>

            <div className="flex flex-wrap items-center gap-4">
              {activeStatusTab === "Sold" && uniqueBatches.length > 0 && (
                <div className="flex items-center gap-2">
                  <span className="text-xs font-bold uppercase tracking-wider text-slate-400">Lọc Lô hàng:</span>
                  <Select
                    value={selectedBatchFilter}
                    onChange={setSelectedBatchFilter}
                    style={{ width: 150 }}
                    dropdownStyle={{ backgroundColor: "#0f172a", border: "1px solid rgba(255,255,255,0.1)" }}
                    className="custom-dark-select h-8"
                  >
                    <Select.Option value="All">Tất cả lô</Select.Option>
                    {uniqueBatches.map((b) => (
                      <Select.Option key={b} value={b}>{b}</Select.Option>
                    ))}
                  </Select>
                </div>
              )}

              <div className="flex items-center gap-2.5">
                <span className="text-xs font-bold uppercase tracking-wider text-slate-400">Lọc Token:</span>
                <Segmented
                  value={subFilter}
                  onChange={setSubFilter}
                  options={[
                    { label: `Tất cả (${countSubFilter("All")})`, value: "All" },
                    { label: `Có Token (${countSubFilter("Login OK")})`, value: "Login OK" },
                    { label: `Chưa Token (${countSubFilter("No Token")})`, value: "No Token" },
                  ]}
                  className="bg-black/25 p-0.5 rounded-[14px]"
                />
              </div>
            </div>
          </div>

          {/* Search & Actions line */}
          <Row gutter={12}>
            <Col xs={24} md={18}>
              <Input
                placeholder="Nhập email cần tìm kiếm..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                prefix={<SearchOutlined className="text-slate-500 mr-1" />}
                allowClear
                className="h-11 rounded-2xl bg-white/5 border-white/10 hover:border-amber-400 focus:border-amber-400 text-white"
              />
            </Col>
            <Col xs={24} md={6} className="mt-3 md:mt-0 flex gap-2">
              <Button className="h-11 rounded-2xl flex-1 border-white/10 text-slate-200 hover:border-amber-400" onClick={loadData}>
                Đồng bộ
              </Button>
            </Col>
          </Row>
        </div>
      </div>

      {/* Table Panel */}
      <div className="glass flex-1 flex flex-col overflow-hidden rounded-[24px] bg-slate-900/40">
        <div className="overflow-auto flex-1 thin-scrollbar">
          <Table
            rowKey="email"
            rowSelection={{
              selectedRowKeys: selectedEmails,
              preserveSelectedRowKeys: true,
              onChange: (keys) => setSelectedEmails(Array.from(new Set(keys))),
            }}
            dataSource={finalFilteredAccounts}
            columns={columns}
            expandable={{
              expandedRowRender,
              rowExpandable: () => true,
            }}
            pagination={{
              pageSize: 10,
              showSizeChanger: true,
              showTotal: (total) => `Tổng cộng ${total} tài khoản`,
            }}
            locale={{
              emptyText: (
                <div className="py-16 text-slate-500 italic">
                  <InboxOutlined style={{ fontSize: 24 }} className="mb-2 block text-slate-600" />
                  Không có tài khoản nào khớp bộ lọc.
                </div>
              ),
            }}
            className="antd-custom-table select-none"
          />
        </div>
      </div>

      {/* Bulk Actions Panel */}
      {selectedEmails.length > 0 && (
        <div className="glass border border-white/5 bg-slate-900/70 rounded-[24px] py-4 px-6">
          <div className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
            <div>
              <p className="text-sm font-semibold text-slate-200">
                Đã chọn <span className="font-mono text-base text-amber-400 font-bold">{selectedEmails.length}</span> tài khoản.
              </p>
              <p className="text-xs text-slate-400 mt-0.5">
                Tác vụ hàng loạt sẽ được áp dụng cho tất cả tài khoản được tích.
              </p>
            </div>
            <Space size={10} wrap>
              <Button
                type="default"
                className="rounded-xl border-white/10 hover:border-rose-400 text-slate-200"
                onClick={() => setSelectedEmails([])}
              >
                Bỏ chọn tất cả
              </Button>
              <Button
                type="dashed"
                className="rounded-xl border-dashed border-white/10 hover:border-amber-400 text-slate-300"
                onClick={async () => {
                  const text = selectedAccounts
                    .map((x) => `${x.email} | ${x.password} | ${x.refreshToken || x.token || ""} | ${x.accountId || ""}`)
                    .join("\n");
                  await navigator.clipboard.writeText(text);
                  message.success(`Đã copy ${selectedEmails.length} acc dạng đầy đủ!`);
                }}
              >
                Copy Đầy Đủ
              </Button>
              <Button
                type="dashed"
                className="rounded-xl border-dashed border-emerald-500/30 hover:border-emerald-400 text-emerald-300"
                loading={isExporting9Router}
                disabled={status === "running"}
                icon={<DownloadOutlined />}
                onClick={handleExportSelected9RouterScripts}
              >
                Export 9Router Scripts
              </Button>
              {last9RouterExportDir && (
                <Button
                  type="default"
                  className="rounded-xl border-white/10 hover:border-sky-400 text-slate-200"
                  onClick={() => handleOpenFolder(last9RouterExportDir)}
                >
                  Open Folder
                </Button>
              )}
              <Button
                type="primary"
                className="rounded-xl border-none bg-slate-800 hover:bg-slate-700 text-slate-200"
                onClick={() => {
                  setSoldTargetEmails(selectedEmails);
                  const today = new Date();
                  const defaultBatch = `Lô ${today.getDate().toString().padStart(2, '0')}/${(today.getMonth() + 1).toString().padStart(2, '0')}`;
                  setBatchName(defaultBatch);
                  setWarrantyDays(3);
                  setSoldModalVisible(true);
                }}
              >
                Mark Sold
              </Button>
              <Button
                type="primary"
                className="bg-gradient-to-r from-amber-500 to-orange-500 hover:from-amber-400 hover:to-orange-400 border-none text-slate-950 font-black rounded-xl shadow-[0_4px_15px_-5px_rgba(245,158,11,0.5)]"
                onClick={() => handleStartAutomation(null, "auto_all")}
              >
                Run Auto ({selectedEmails.length})
              </Button>
              <Dropdown menu={bulkStepMenu} trigger={["click"]} disabled={status === "running"}>
                <Button
                  type="primary"
                  className="bg-sky-500 hover:bg-sky-400 border-none text-slate-950 font-bold rounded-xl"
                  disabled={status === "running"}
                >
                  Chạy riêng Step
                </Button>
              </Dropdown>
              <Button
                type="primary"
                danger
                className="bg-amber-600 hover:bg-amber-500 border-none text-slate-950 font-bold rounded-xl"
                onClick={() => handleStartAutomation()}
              >
                Run Step ({selectedEmails.length})
              </Button>
            </Space>
          </div>
      )}
      <Modal
        title={
          <div className="flex items-center gap-2 text-white font-bold text-lg border-b border-white/10 pb-3">
            <DollarOutlined className="text-cyan-400" />
            <span>Đánh dấu "Đã bán" & Cấu hình bảo hành</span>
          </div>
        }
        open={soldModalVisible}
        onOk={async () => {
          if (!batchName.trim()) {
            message.warning("Vui lòng nhập tên lô hàng!");
            return;
          }
          await markMultipleAccountsStatus(soldTargetEmails, "Sold", batchName.trim(), warrantyDays);
          setSelectedEmails([]);
          setSoldModalVisible(false);
        }}
        onCancel={() => setSoldModalVisible(false)}
        okText="Xác nhận"
        cancelText="Hủy"
        className="custom-dark-modal"
        styles={{
          body: { backgroundColor: "#0f172a", color: "#f8fafc", padding: "20px 24px" },
          content: { backgroundColor: "#0f172a", border: "1px solid rgba(255,255,255,0.1)", borderRadius: "20px" },
          header: { backgroundColor: "#0f172a", padding: "16px 24px 0 24px" },
          footer: { backgroundColor: "#0f172a", borderTop: "1px solid rgba(255,255,255,0.05)", padding: "12px 24px" }
        }}
      >
        <div className="space-y-5 text-slate-300">
          <p className="text-xs text-slate-400">
            Bạn đang đánh dấu <span className="font-semibold text-cyan-400">{soldTargetEmails.length}</span> tài khoản là <span className="font-semibold text-cyan-400">Đã bán</span>. Vui lòng thiết lập thông tin lô hàng và bảo hành để dễ dàng đối soát.
          </p>
          
          <div className="space-y-2">
            <label className="text-xs font-bold uppercase tracking-wider text-slate-400 block">Tên Lô Hàng (Tracing Batch)</label>
            <Input
              value={batchName}
              onChange={(e) => setBatchName(e.target.value)}
              placeholder="Ví dụ: Lô 26/05, Lô VIP, v.v."
              className="h-10 rounded-xl bg-white/5 border-white/10 text-white focus:border-cyan-500 hover:border-cyan-500 placeholder:text-slate-600"
            />
          </div>

          <div className="space-y-2">
            <label className="text-xs font-bold uppercase tracking-wider text-slate-400 block">Thời gian bảo hành (Ngày)</label>
            <div className="flex items-center gap-4">
              <InputNumber
                min={1}
                max={30}
                value={warrantyDays}
                onChange={(val) => setWarrantyDays(Math.max(1, Number(val) || 1))}
                className="w-24 rounded-xl bg-white/5 border-white/10 text-white focus:border-cyan-500 hover:border-cyan-500"
              />
              <span className="text-xs text-slate-400 font-medium">mặc định là 3 ngày bảo hành đếm ngược.</span>
            </div>
          </div>
        </div>
      </Modal>
    </div>
  );
}
