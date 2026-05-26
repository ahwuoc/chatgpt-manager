import React from "react";
import { Card, Progress, Row, Col, Space, Button } from "antd";
import {
  TeamOutlined,
  ChromeOutlined,
  CheckCircleOutlined,
  DollarOutlined,
  PlayCircleOutlined,
} from "@ant-design/icons";
import { cn } from "../lib/utils";

export default function DashboardView({
  accounts,
  linkedPaypalCount,
  successRate,
  countStats,
  setActiveWorkflow,
  handleStartAutomation,
}) {
  const dashboardStats = [
    {
      icon: TeamOutlined,
      label: "Tổng tài khoản",
      value: accounts.length,
      hint: "Toàn bộ account đã nạp",
      color: "text-blue-400",
    },
    {
      icon: ChromeOutlined,
      label: "PayPal links",
      value: linkedPaypalCount,
      hint: "Link approve đã sẵn sàng",
      valueClassName: "text-amber-400",
      color: "text-amber-400",
    },
    {
      icon: CheckCircleOutlined,
      label: "Reg Trial Success",
      value: countStats("Success"),
      hint: `${successRate}% conversion`,
      valueClassName: "text-emerald-400",
      color: "text-emerald-400",
    },
    {
      icon: DollarOutlined,
      label: "Sold Out",
      value: countStats("Sold"),
      hint: `${countStats("Fail")} account đang fail`,
      valueClassName: "text-rose-400",
      color: "text-rose-400",
    },
  ];

  return (
    <div className="flex flex-col gap-6 p-2">
      {/* Statistics Cards */}
      <Row gutter={[16, 16]}>
        {dashboardStats.map((item) => (
          <Col xs={24} sm={12} xl={6} key={item.label}>
            <div className="glass hover:bg-slate-900/40 transition-all duration-300 rounded-[24px] p-5 flex flex-col justify-between h-40 relative overflow-hidden">
              <div className="flex items-start justify-between">
                <div>
                  <p className="text-[10px] font-bold uppercase tracking-[0.2em] text-slate-400">{item.label}</p>
                  <p className={cn("text-3xl font-extrabold tracking-tight text-white mt-2", item.valueClassName)}>
                    {item.value}
                  </p>
                </div>
                <div className={cn("rounded-2xl border border-white/5 bg-white/5 p-3", item.color)}>
                  <item.icon style={{ fontSize: 20 }} />
                </div>
              </div>
              {item.hint && <p className="text-xs text-slate-400 mt-2">{item.hint}</p>}
            </div>
          </Col>
        ))}
      </Row>

      {/* Action Panels */}
      <Row gutter={[20, 20]} className="mt-2">
        <Col xs={24} xl={15}>
          <div className="glass rounded-[24px] p-6 h-full">
            <h3 className="text-lg font-bold text-white mb-5 flex items-center gap-2">
              <PlayCircleOutlined className="text-amber-400" />
              Chạy Nhanh Quy Trình
            </h3>
            <Row gutter={[12, 12]}>
              <Col span={24} md={8}>
                <button
                  onClick={() => {
                    handleStartAutomation(null, "auto_all");
                  }}
                  className="w-full text-left rounded-2xl border border-amber-500/20 bg-amber-500/5 p-4 transition hover:border-amber-400/40 hover:bg-amber-500/10 group duration-300 flex flex-col justify-between h-48 shadow-[0_4px_20px_-10px_rgba(245,158,11,0.3)]"
                >
                  <div>
                    <span className="text-[15px] font-extrabold text-amber-400 block group-hover:text-amber-300 transition">
                      🔥 Chạy Auto Trọn Gói
                    </span>
                    <p className="mt-1.5 text-[11px] leading-relaxed text-slate-400 group-hover:text-slate-300 transition">
                      Tự động hóa hoàn toàn từ đầu đến cuối: Login lấy Token ➡️ Tạo link Stripe ➡️ Duyệt PayPal SMS/OTP!
                    </p>
                  </div>
                  <span className="text-[11px] text-amber-500 font-bold mt-3 block group-hover:translate-x-1 transition-transform">
                    Kích hoạt trọn gói →
                  </span>
                </button>
              </Col>

              <Col span={24} md={8}>
                <button
                  onClick={() => {
                    handleStartAutomation(null, "auth");
                  }}
                  className="w-full text-left rounded-2xl border border-sky-500/10 bg-sky-500/5 p-4 transition hover:border-sky-400/30 hover:bg-sky-500/10 group duration-300 flex flex-col justify-between h-48"
                >
                  <div>
                    <span className="text-[15px] font-bold text-sky-400 block group-hover:text-sky-300 transition">
                      Bước 1: Login OTP
                    </span>
                    <p className="mt-1.5 text-[11px] leading-relaxed text-slate-400 group-hover:text-slate-300 transition">
                      Chạy trực tiếp `auth.rs` để đăng nhập Chrome và lấy Access Token/Session cho toàn bộ danh sách.
                    </p>
                  </div>
                  <span className="text-[11px] text-sky-500 font-semibold mt-3 block group-hover:translate-x-1 transition-transform">
                    Khởi chạy ngay →
                  </span>
                </button>
              </Col>

              <Col span={24} md={8}>
                <button
                  onClick={() => {
                    handleStartAutomation(null, "paypal_approve");
                  }}
                  className="w-full text-left rounded-2xl border border-orange-500/10 bg-orange-500/5 p-4 transition hover:border-orange-400/30 hover:bg-orange-500/10 group duration-300 flex flex-col justify-between h-48"
                >
                  <div>
                    <span className="text-[15px] font-bold text-orange-400 block group-hover:text-orange-300 transition">
                      Bước 4: Duyệt PayPal
                    </span>
                    <p className="mt-1.5 text-[11px] leading-relaxed text-slate-400 group-hover:text-slate-300 transition">
                      Chạy trực tiếp `paypal_approve.rs` để mở trình duyệt, tự động điền form thẻ và nhận OTP SMS để hoàn tất.
                    </p>
                  </div>
                  <span className="text-[11px] text-orange-500 font-semibold mt-3 block group-hover:translate-x-1 transition-transform">
                    Khởi chạy ngay →
                  </span>
                </button>
              </Col>
            </Row>
          </div>
        </Col>

        <Col xs={24} xl={9}>
          <div className="glass rounded-[24px] p-6 h-full flex flex-col justify-between">
            <div>
              <h3 className="text-lg font-bold text-white mb-4">Theo Dõi Pipeline</h3>
              <div className="rounded-xl border border-white/5 bg-white/5 p-4 mb-5">
                <span className="text-[10px] font-bold uppercase tracking-wider text-slate-400 block">
                  Hiệu suất kích hoạt
                </span>
                <span className="text-2xl font-black text-emerald-400 mt-2 block font-mono">
                  {successRate}%
                </span>
                <p className="text-xs text-slate-400 leading-normal mt-1.5">
                  Tỷ lệ chuyển đổi tài khoản ChatGPT Plus thành công trên tổng số tài khoản hiện có trong hệ thống.
                </p>
              </div>

              <div className="space-y-2 mb-5">
                <div className="flex items-center justify-between text-xs text-slate-400">
                  <span>Tiến độ tổng quan</span>
                  <span className="font-mono text-white">{successRate}%</span>
                </div>
                <Progress
                  percent={successRate}
                  strokeColor={{
                    "0%": "#10b981",
                    "100%": "#3b82f6",
                  }}
                  trailColor="rgba(255,255,255,0.05)"
                  showInfo={false}
                  className="m-0"
                />
              </div>
            </div>

            <Space size={6} wrap>
              <span className="inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-xs font-semibold bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
                Thành công: {countStats("Success")}
              </span>
              <span className="inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-xs font-semibold bg-rose-500/10 text-rose-400 border border-rose-500/20">
                Lỗi/Fail: {countStats("Fail")}
              </span>
              <span className="inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-xs font-semibold bg-amber-500/10 text-amber-400 border border-amber-500/20">
                Chờ xử lý: {countStats("Pending")}
              </span>
              <span className="inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-xs font-semibold bg-slate-500/10 text-slate-400 border border-slate-500/20">
                Đã bán: {countStats("Sold")}
              </span>
            </Space>
          </div>
        </Col>
      </Row>
    </div>
  );
}
