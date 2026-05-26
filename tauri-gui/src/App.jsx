import React from "react";
import { ConfigProvider, theme as antTheme, Button, Tag } from "antd";
import {
  TeamOutlined,
  FileTextOutlined,
  SettingOutlined,
  ReloadOutlined,
  StopOutlined,
} from "@ant-design/icons";
import { cn } from "./lib/utils";
import { useConsoleState } from "./hooks/useConsoleState";
import AccountsPage from "./pages/AccountsPage";
import LogsPage from "./pages/LogsPage";
import SettingsPage from "./pages/SettingsPage";
import Modals from "./components/Modals";

const NAV_ITEMS = [
  { key: "accounts", label: "Account Management", icon: TeamOutlined },
  { key: "logs", label: "Automation Live Logs", icon: FileTextOutlined },
  { key: "settings", label: "System Config", icon: SettingOutlined },
];

function App() {
  const consoleState = useConsoleState(NAV_ITEMS);

  const renderActivePage = () => {
    if (consoleState.navTab === "accounts") {
      return <AccountsPage consoleState={consoleState} />;
    }
    if (consoleState.navTab === "logs") {
      return <LogsPage consoleState={consoleState} />;
    }
    if (consoleState.navTab === "settings") {
      return <SettingsPage consoleState={consoleState} />;
    }
    return null;
  };

  return (
    <ConfigProvider
      theme={{
        algorithm: antTheme.darkAlgorithm,
        token: {
          colorPrimary: "#f59e0b",
          borderRadius: 20,
          fontFamily: "'Outfit', 'Inter', system-ui, -apple-system, sans-serif",
          colorBgContainer: "rgba(10, 15, 30, 0.45)",
          colorBgElevated: "#090d16",
        },
      }}
    >
      <div className="flex h-screen w-screen overflow-hidden text-slate-100 bg-brand-dark">
        <aside className="flex w-24 flex-shrink-0 flex-col justify-between border-r border-white/5 bg-slate-950/60 p-4 backdrop-blur-xl">
          <div className="space-y-6">
            <div className="rounded-3xl border border-white/10 bg-gradient-to-br from-amber-300 via-amber-200 to-orange-500 p-2.5 text-slate-950 shadow-[0_16px_60px_-30px_rgba(245,158,11,0.8)]">
              <div className="rounded-2xl bg-black/10 py-2.5 text-center">
                <p className="text-[10px] font-black uppercase tracking-[0.2em] leading-none">Auto</p>
                <p className="mt-1 text-2xl font-black leading-none">C</p>
              </div>
            </div>

            <div className="space-y-2">
              {NAV_ITEMS.map(({ key, label, icon: Icon }) => (
                <Button
                  key={key}
                  type={consoleState.navTab === key ? "primary" : "text"}
                  onClick={() => consoleState.setNavTab(key)}
                  title={label}
                  className={cn(
                    "h-12 w-12 rounded-2xl flex items-center justify-center mx-auto border-none",
                    consoleState.navTab === key
                      ? "bg-amber-500 hover:bg-amber-400 text-slate-950"
                      : "text-slate-400 hover:bg-white/5",
                  )}
                  icon={<Icon style={{ fontSize: 18 }} />}
                />
              ))}
            </div>
          </div>

          <div className="space-y-3">
            {consoleState.status === "running" ? (
              <Button
                type="primary"
                danger
                onClick={consoleState.handleStopAutomation}
                title="Dừng khẩn cấp"
                className="w-full rounded-2xl flex items-center justify-center bg-rose-600 hover:bg-rose-500 border-none text-white h-11"
                icon={<StopOutlined />}
              />
            ) : (
              <Tag color="success" className="w-full text-center py-1.5 rounded-full font-bold uppercase text-[9px] tracking-wider border-none bg-emerald-500/10 text-emerald-400">
                Ready
              </Tag>
            )}
            <div className="rounded-2xl border border-white/5 bg-white/5 p-3 text-center">
              <p className="text-[10px] font-semibold uppercase tracking-[0.2em] text-slate-400">Select</p>
              <p className="mt-1 font-mono text-lg text-white font-bold">{consoleState.selectedEmails.length}</p>
            </div>
            <p className="text-center font-mono text-[9px] text-slate-500">v1.2</p>
          </div>
        </aside>

        <section className="flex min-w-0 flex-1 flex-col overflow-hidden bg-brand-dark">
          <header className="border-b border-white/5 bg-slate-950/35 px-8 py-5 backdrop-blur-xl">
            <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
              <div>
                <p className="text-[10px] font-bold uppercase tracking-[0.25em] text-slate-500">
                  ChatGPT premium activation console
                </p>
                <h1 className="mt-1.5 text-2xl font-bold tracking-tight text-white">
                  {consoleState.currentNav?.label}
                </h1>
              </div>

              <div className="flex flex-wrap items-center gap-3">
                <Tag color="cyan" className="px-3 py-1.5 rounded-full text-xs font-semibold border-none bg-cyan-500/10 text-cyan-400">
                  {consoleState.selectedEmails.length} accounts selected
                </Tag>
                {consoleState.selectedEmails.length > 0 && (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => consoleState.setSelectedEmails([])}
                    className="rounded-full border-white/10 hover:border-rose-400 text-slate-300"
                  >
                    Clear select
                  </Button>
                )}
                <Button
                  variant="outline"
                  size="sm"
                  onClick={consoleState.loadData}
                  className="rounded-full border-white/10 hover:border-amber-400 text-slate-300"
                >
                  <ReloadOutlined /> Sync backend
                </Button>
              </div>
            </div>
          </header>

          <div className="flex-1 overflow-hidden">
            {renderActivePage()}
          </div>
        </section>

        <Modals
          showImportModal={consoleState.showImportModal}
          setShowImportModal={consoleState.setShowImportModal}
          bulkText={consoleState.bulkText}
          setBulkText={consoleState.setBulkText}
          handleImportBulk={consoleState.handleImportBulk}
        />
      </div>
    </ConfigProvider>
  );
}

export default App;
