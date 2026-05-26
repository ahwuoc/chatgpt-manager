import React, { useState, useEffect } from "react";
import { Button, Input, Switch, Spin, message, Popconfirm } from "antd";
import { invoke } from "@tauri-apps/api/core";
import { ReloadOutlined, SaveOutlined, DeleteOutlined, ChromeOutlined } from "@ant-design/icons";

function parseProxyLine(proxyLine) {
  const raw = (proxyLine || "").trim();
  if (!raw) {
    return { host: "", port: "", user: "", pass: "", auth: "None" };
  }

  const withoutScheme = raw.replace(/^https?:\/\//i, "").replace(/\/$/, "");
  let hostPort = withoutScheme;
  let user = "";
  let pass = "";

  if (withoutScheme.includes("@")) {
    const [credentials, target] = withoutScheme.split("@");
    hostPort = target;
    const [parsedUser = "", parsedPass = ""] = credentials.split(":");
    user = parsedUser;
    pass = parsedPass;
  } else {
    const parts = withoutScheme.split(":");
    if (parts.length >= 4) {
      hostPort = `${parts[0]}:${parts[1]}`;
      user = parts[2] || "";
      pass = parts.slice(3).join(":");
    }
  }

  const [host = "", port = ""] = hostPort.split(":");
  return {
    host,
    port,
    user,
    pass,
    auth: user || pass ? "User:Password" : "No Auth",
  };
}

export default function SystemConfigView({
  phone,
  setPhone,
  status,
  autoPipeline,
  setAutoPipeline,
  handleSaveSettings,
}) {
  const [manualPhone, setManualPhone] = useState("");
  const [otpRelayUrl, setOtpRelayUrl] = useState("");
  const [usProxyEnabled, setUsProxyEnabled] = useState(false);
  const [usProxyLabel, setUsProxyLabel] = useState("");
  const [usProxyLine, setUsProxyLine] = useState("");
  const [usProxyChangeUrl, setUsProxyChangeUrl] = useState("");
  const [usProxyRotate, setUsProxyRotate] = useState(false);
  const [usProxyStatus, setUsProxyStatus] = useState(null);
  const [checkingUsProxy, setCheckingUsProxy] = useState(false);
  const [changingUsProxy, setChangingUsProxy] = useState(false);

  const [loadingConfig, setLoadingConfig] = useState(false);
  const [cleaningProfiles, setCleaningProfiles] = useState(false);

  // Load Config on mount
  useEffect(() => {
    async function loadConfig() {
      setLoadingConfig(true);
      try {
        const config = await invoke("get_sms_config");
        setManualPhone(config.manual_phone || phone || "");
        setOtpRelayUrl(config.otp_relay_url || "");

        const proxyConfig = await invoke("get_us_browser_proxy_config");
        setUsProxyEnabled(proxyConfig.enabled || false);
        setUsProxyLabel(proxyConfig.label || "");
        setUsProxyLine(proxyConfig.proxy || "");
        setUsProxyChangeUrl(proxyConfig.change_ip_url || "");
        setUsProxyRotate(proxyConfig.rotate_ip_before_launch || false);
      } catch (e) {
        console.error("Lỗi tải cấu hình hệ thống:", e);
      } finally {
        setLoadingConfig(false);
      }
    }
    loadConfig();
  }, []);

  const handleSaveSmsConfig = async () => {
    try {
      await invoke("save_sms_config", {
        config: {
          manual_phone: manualPhone.trim(),
          otp_relay_url: otpRelayUrl.trim(),
        }
      });
      return true;
    } catch (e) {
      message.error("Lỗi lưu cấu hình SMS & Phone: " + e);
      return false;
    }
  };

  const handleSaveUsProxyConfig = async () => {
    const finalProxy = usProxyLine.trim();
    if (usProxyEnabled && !finalProxy) {
      message.error("Bật browser proxy thì cần nhập HTTP IPv4 Proxy!");
      return false;
    }

    try {
      await invoke("save_us_browser_proxy_config", {
        config: {
          enabled: usProxyEnabled,
          label: usProxyLabel.trim(),
          proxy: finalProxy,
          change_ip_url: usProxyChangeUrl.trim(),
          rotate_ip_before_launch: usProxyRotate,
        },
      });
      return true;
    } catch (e) {
      message.error("Lỗi lưu Browser Proxy: " + e);
      return false;
    }
  };

  const handleSaveAll = async () => {
    await handleSaveSettings();
    const sSave = await handleSaveSmsConfig();
    const proxySave = await handleSaveUsProxyConfig();
    if (sSave && proxySave) {
      message.success("💾 Đã lưu toàn bộ cấu hình hệ thống thành công!");
    }
  };

  const handleCleanupChromeProfiles = async () => {
    setCleaningProfiles(true);
    try {
      const result = await invoke("cleanup_chrome_profiles");
      message.success(
        `Đã dọn Chrome profiles: ${result.removedFiles} file, ${result.removedDirs} thư mục.`
      );
    } catch (e) {
      message.error("Lỗi dọn Chrome profiles: " + e);
    } finally {
      setCleaningProfiles(false);
    }
  };

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

  useEffect(() => {
    refreshUsProxyStatus(true);
  }, []);

  const usProxyInfo = parseProxyLine(usProxyLine);
  const proxyEnabled = usProxyStatus?.enabled ?? usProxyEnabled;
  const proxyEndpoint = usProxyStatus?.host
    ? `${usProxyStatus.host}${usProxyStatus.port ? `:${usProxyStatus.port}` : ""}`
    : (usProxyInfo.host ? `${usProxyInfo.host}${usProxyInfo.port ? `:${usProxyInfo.port}` : ""}` : "Chưa cấu hình");
  const proxyExitIp = usProxyStatus?.currentIp || (proxyEnabled ? "Chưa check" : "Đang tắt");
  const proxyLocation = [
    usProxyStatus?.city,
    usProxyStatus?.region,
    usProxyStatus?.countryCode || usProxyStatus?.country,
  ].filter(Boolean).join(", ");
  const proxyNetwork = usProxyStatus?.isp || usProxyStatus?.asn || "";

  return (
    <div className="flex h-full flex-col gap-6 p-2 overflow-y-auto">
      <div className="w-full space-y-6">
        <div className="grid grid-cols-1 xl:grid-cols-2 gap-6 items-start">
          
          {/* ─── PHẦN 1: CẤU HÌNH SMS & PHONE ─── */}
          <div className="glass rounded-[24px] p-6 border border-white/5 bg-slate-900/40 space-y-6">
            <div className="flex items-center justify-between border-b border-white/5 pb-4">
              <h3 className="text-lg font-bold text-white flex items-center gap-2">
                Cấu hình SMS & Phone
              </h3>
              {loadingConfig && <Spin size="small" />}
            </div>

            <div className="space-y-4">
              <div className="text-xs font-black uppercase tracking-[0.2em] text-amber-400">
                Nhận tin nhắn OTP & Phone điền thẻ
              </div>

              {/* Phone thủ công */}
              <div className="space-y-1.5">
                <label className="text-xs font-bold text-slate-300">Số điện thoại thủ công</label>
                <Input
                  value={manualPhone}
                  onChange={(e) => setManualPhone(e.target.value)}
                  placeholder="Ví dụ: 3506003242 hoặc +13506003242"
                  className="h-11 rounded-2xl bg-white/5 border-white/10 text-white font-mono hover:border-amber-400 focus:border-amber-400"
                />
                <p className="text-[11px] text-slate-500">
                  Số điện thoại Mỹ dùng để điền form thanh toán PayPal (bỏ trống sẽ tự động sinh số ngẫu nhiên).
                </p>
              </div>

              {/* Relay API URL */}
              <div className="space-y-1.5">
                <label className="text-xs font-bold text-slate-300">URL API nhận mã OTP</label>
                <Input
                  value={otpRelayUrl}
                  onChange={(e) => setOtpRelayUrl(e.target.value)}
                  placeholder="Nhập URL API để nhận mã OTP SMS..."
                  className="h-11 rounded-2xl bg-white/5 border-white/10 text-white font-mono hover:border-amber-400 focus:border-amber-400"
                />
                <p className="text-[11px] text-slate-500">
                  Đường dẫn API lấy tin nhắn SMS chứa mã OTP tự động (mặc định sử dụng Yuecheng Relay).
                </p>
              </div>
            </div>
          </div>

          {/* ─── PHẦN 2: BROWSER PROXY ─── */}
          <div className="space-y-6">
            <div className="glass rounded-[24px] p-6 border border-sky-500/10 bg-slate-900/40 space-y-6">
              <div className="flex items-center justify-between border-b border-white/5 pb-4">
                <h3 className="text-lg font-bold text-white">Browser Proxy</h3>
                <span className="text-[11px] font-bold uppercase tracking-[0.16em] text-sky-400">
                  Auth & PayPal
                </span>
              </div>

              <div className="space-y-4 p-4 rounded-2xl border border-sky-500/10 bg-sky-500/5">
                <div className="flex flex-col gap-3 rounded-2xl border border-sky-500/10 bg-sky-500/5 px-4 py-3 lg:flex-row lg:items-center lg:justify-between">
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="flex items-center gap-1.5 text-sm font-bold text-sky-200">
                        <ChromeOutlined className="text-sky-400" />
                        Browser Proxy
                      </span>
                      <span className={`rounded-full px-2.5 py-0.5 text-[10px] font-bold uppercase tracking-wider ${proxyEnabled ? "bg-sky-500/20 text-sky-300" : "bg-white/10 text-slate-300"}`}>
                        {proxyEnabled ? "Bật" : "Tắt"}
                      </span>
                      {proxyEnabled && (usProxyStatus?.countryCode || usProxyStatus?.country) && (
                        <span className="rounded-full bg-blue-500/20 px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider text-blue-300">
                          {usProxyStatus.countryCode || usProxyStatus.country}
                        </span>
                      )}
                    </div>
                    <div className="mt-1 flex flex-wrap items-center gap-x-4 gap-y-1 text-[11px] text-slate-400 font-mono">
                      <span>Host: {proxyEndpoint}</span>
                      <span className="text-sky-300 font-bold">Exit IP: {proxyExitIp}</span>
                      {proxyLocation && <span>Vị trí: {proxyLocation}</span>}
                      {proxyNetwork && <span className="max-w-[360px] truncate">Mạng: {proxyNetwork}</span>}
                      {usProxyStatus?.waitSeconds ? (
                        <span className="text-amber-300">Chờ {usProxyStatus.waitSeconds}s</span>
                      ) : (
                        usProxyStatus?.message && <span className="max-w-[520px] truncate">{usProxyStatus.message}</span>
                      )}
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
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
                  </div>
                </div>

                <div className="flex flex-col md:flex-row md:items-center md:justify-between gap-4">
                  <div className="min-w-0">
                    <span className="text-[13px] font-bold text-sky-300 block font-sans">
                      Proxy cho Auth & PayPal Approve
                    </span>
                    <p className="text-[11px] text-slate-400 mt-1 leading-relaxed">
                      {usProxyEnabled ? "Đang bật cho browser automation." : "Đang tắt, browser dùng mạng mặc định."}
                    </p>
                  </div>
                  <Switch
                    checked={usProxyEnabled}
                    onChange={setUsProxyEnabled}
                    activeBg="#38bdf8"
                  />
                </div>

                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div className="space-y-1.5">
                    <label className="text-xs font-bold text-slate-300">Nhãn proxy</label>
                    <Input
                      value={usProxyLabel}
                      onChange={(e) => setUsProxyLabel(e.target.value)}
                      placeholder="EN8895_1208"
                      className="h-11 rounded-2xl bg-white/5 border-white/10 text-white hover:border-sky-400 focus:border-sky-400"
                    />
                  </div>

                  <div className="space-y-1.5">
                    <label className="text-xs font-bold text-slate-300">Đổi IP theo từng profile</label>
                    <div className="h-11 px-4 rounded-2xl bg-white/5 border border-white/10 flex items-center justify-between">
                      <span className="text-xs text-slate-400">{usProxyRotate ? "Mỗi profile đổi 1 lần" : "Giữ IP hiện tại"}</span>
                      <Switch
                        checked={usProxyRotate}
                        onChange={setUsProxyRotate}
                        disabled={!usProxyEnabled}
                        activeBg="#38bdf8"
                      />
                    </div>
                  </div>
                </div>

                <div className="space-y-1.5">
                  <label className="text-xs font-bold text-slate-300">HTTP IPv4 Proxy</label>
                  <Input.Password
                    value={usProxyLine}
                    onChange={(e) => setUsProxyLine(e.target.value)}
                    placeholder="host:port:user:pass"
                    className="h-11 rounded-2xl bg-white/5 border-white/10 text-white font-mono hover:border-sky-400 focus:border-sky-400"
                  />
                </div>

                <div className="space-y-1.5">
                  <label className="text-xs font-bold text-slate-300">Link change IP</label>
                  <Input
                    value={usProxyChangeUrl}
                    onChange={(e) => setUsProxyChangeUrl(e.target.value)}
                    placeholder="https://api.enode.vn/getip/us/..."
                    className="h-11 rounded-2xl bg-white/5 border-white/10 text-white font-mono hover:border-sky-400 focus:border-sky-400"
                  />
                </div>

                <div className="grid grid-cols-2 md:grid-cols-4 gap-2">
                  {[
                    ["Host", usProxyInfo.host || "-"],
                    ["HTTP", usProxyInfo.port || "-"],
                    ["Auth", usProxyInfo.auth],
                    ["User", usProxyInfo.user || "-"],
                  ].map(([label, value]) => (
                    <div key={label} className="rounded-xl bg-slate-950/30 border border-white/5 px-3 py-2 min-w-0">
                      <div className="text-[10px] uppercase tracking-[0.12em] text-slate-500">{label}</div>
                      <div className="text-xs font-semibold text-slate-200 truncate font-mono">{value}</div>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* ─── PIPELINE TỰ ĐỘNG & DỌN DẸP ─── */}
        <div className="grid grid-cols-1 xl:grid-cols-2 gap-6">
          <div className="glass rounded-[24px] p-6 border border-amber-500/10 bg-slate-900/40 space-y-4">
            <div className="space-y-4">
              <div className="text-xs font-black uppercase tracking-[0.2em] text-slate-400">
                Luồng chạy tự động
              </div>

              <div className="flex items-center justify-between p-4 rounded-2xl border border-amber-500/10 bg-amber-500/5 gap-6">
                <div className="max-w-[80%]">
                  <span className="text-[13px] font-bold text-amber-400 block">
                    🔥 Chạy Tự Động Liên Tục (Auto Pipeline)
                  </span>
                  <p className="text-[11px] text-slate-400 mt-1 leading-relaxed">
                    Khi bật, tất cả các tác vụ chạy (Run hoặc Chạy Smart Batch) sẽ tự động chạy liên hoàn: Đăng nhập ➡️ Khởi tạo trang thanh toán ➡️ Điền thẻ và duyệt PayPal tự động đóng trình duyệt khi thành công/thất bại, không cần ấn tay từng bước nữa.
                  </p>
                </div>
                <Switch
                  checked={autoPipeline}
                  onChange={(val) => setAutoPipeline(val)}
                  activeBg="#f59e0b"
                />
              </div>
            </div>
          </div>

          <div className="glass rounded-[24px] p-6 border border-red-500/10 bg-slate-900/40 space-y-4">
            <div className="space-y-4">
              <div className="text-xs font-black uppercase tracking-[0.2em] text-slate-400">
                Dọn dẹp trình duyệt
              </div>

              <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between p-4 rounded-2xl border border-red-500/10 bg-red-500/5 gap-4">
                <div className="min-w-0">
                  <span className="text-[13px] font-bold text-red-300 block">
                    Dọn Chrome profiles
                  </span>
                  <p className="text-[11px] text-slate-400 mt-1 leading-relaxed break-all">
                    Xóa dữ liệu tại /home/ahwuocdz/rust-chatgpt/tauri-gui/src-tauri/data/chrome_profiles
                  </p>
                </div>

                <Popconfirm
                  title="Xóa toàn bộ Chrome profiles?"
                  description="Hãy dừng automation và đóng trình duyệt trước khi dọn."
                  okText="Xóa"
                  cancelText="Hủy"
                  okButtonProps={{ danger: true }}
                  onConfirm={handleCleanupChromeProfiles}
                >
                  <Button
                    danger
                    loading={cleaningProfiles}
                    icon={<DeleteOutlined />}
                    className="h-11 px-5 rounded-2xl font-bold bg-red-500/10 hover:bg-red-500/20 border-red-500/30 text-red-300 hover:text-red-200"
                  >
                    Dọn Profiles
                  </Button>
                </Popconfirm>
              </div>
            </div>
          </div>
        </div>

        {/* ─── ACTION FOOTER ─── */}
        <div className="glass rounded-[24px] p-4 border border-white/5 bg-slate-900/40 flex flex-col sm:flex-row gap-3">
          <Button
            type="primary"
            onClick={handleSaveAll}
            icon={<SaveOutlined />}
            className="w-full sm:w-auto h-11 px-8 font-bold rounded-2xl bg-amber-500 hover:bg-amber-400 border-none text-slate-950 flex items-center justify-center gap-2"
          >
            Lưu Cấu Hình Hệ Thống
          </Button>
        </div>
      </div>
    </div>
  );
}
