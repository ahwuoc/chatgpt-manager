import { useEffect, useRef } from "react";
import {
  Play,
  CreditCard,
  Sparkles,
  Activity,
  Users,
  Download,
  BadgeCheck,
  LoaderCircle,
} from "lucide-react";
import { Button } from "./ui/button";

export function AccountActionDropdown({
  acc,
  isOpen,
  onClose,
  nextStep,
  onStartAutomation,
  onTriggerGetOTP,
  onMarkAccountStatus,
}) {
  const dropdownRef = useRef(null);

  useEffect(() => {
    if (!isOpen) return;

    function handleClickOutside(event) {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target)) {
        onClose();
      }
    }

    const timer = setTimeout(() => {
      document.addEventListener("mousedown", handleClickOutside);
    }, 50);

    return () => {
      clearTimeout(timer);
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  return (
    <div
      ref={dropdownRef}
      className="absolute right-0 top-full z-50 mt-2 w-72 origin-top-right rounded-2xl border border-white/10 bg-slate-950/95 p-3.5 backdrop-blur-xl shadow-[0_20px_50px_rgba(0,0,0,0.6)] animate-in fade-in slide-in-from-top-2 duration-150"
    >
      <div className="space-y-4">

        {/* Section 1: Automation Workflows */}
        <div>
          <p className="px-2 pb-2 text-[10px] font-bold uppercase tracking-[0.2em] text-slate-500 flex items-center gap-1.5">
            <Play className="h-3 w-3 text-slate-400" />
            Quy trình chạy
          </p>
          <div className="space-y-1">
            <Button
              variant="ghost"
              size="sm"
              onClick={() => {
                onClose();
                onStartAutomation(acc.email, nextStep);
              }}
              className="h-9 w-full justify-start rounded-xl px-2.5 hover:bg-white/5 text-slate-300 transition duration-100"
            >
              <Play className="mr-2 h-3.5 w-3.5 text-emerald-400" />
              <span className="truncate text-xs">Chạy bước tiếp theo</span>
            </Button>

            <Button
              variant="ghost"
              size="sm"
              onClick={() => {
                onClose();
                onStartAutomation(acc.email, "make_payment");
              }}
              className="h-9 w-full justify-start rounded-xl px-2.5 hover:bg-white/5 text-slate-300 transition duration-100"
            >
              <CreditCard className="mr-2 h-3.5 w-3.5 text-sky-400" />
              <span className="truncate text-xs">Tạo lại Link Payment</span>
            </Button>

            <Button
              variant="ghost"
              size="sm"
              onClick={() => {
                onClose();
                onStartAutomation(acc.email, "paypal_approve");
              }}
              className="h-9 w-full justify-start rounded-xl px-2.5 hover:bg-white/5 text-slate-300 transition duration-100"
            >
              <Sparkles className="mr-2 h-3.5 w-3.5 text-amber-400" />
              <span className="truncate text-xs">Chạy lại Duyệt PayPal</span>
            </Button>
          </div>
        </div>

        <div className="h-px bg-white/5" />

        {/* Section 2: Data & Utilities */}
        <div>
          <p className="px-2 pb-2 text-[10px] font-bold uppercase tracking-[0.2em] text-slate-500 flex items-center gap-1.5">
            <Download className="h-3 w-3 text-slate-400" />
            Dữ liệu & Tiện ích
          </p>
          <div className="space-y-1">
            <Button
              variant="ghost"
              size="sm"
              onClick={async () => {
                onClose();
                const text = `${acc.email} | ${acc.password} | ${acc.refreshToken || acc.token || ""} | ${acc.accountId || ""}`;
                try {
                  await navigator.clipboard.writeText(text);
                  alert(`Đã copy tài khoản ${acc.email} dạng đầy đủ!`);
                } catch (err) {
                  alert("Lỗi sao chép: " + err);
                }
              }}
              className="h-9 w-full justify-start rounded-xl px-2.5 hover:bg-white/5 text-slate-300 transition duration-100"
            >
              <Users className="mr-2 h-3.5 w-3.5 text-indigo-400" />
              <span className="truncate text-xs">Copy Acc Đầy Đủ</span>
            </Button>

            <Button
              variant="ghost"
              size="sm"
              onClick={() => {
                onClose();
                onTriggerGetOTP(acc.email, acc.password);
              }}
              className="h-9 w-full justify-start rounded-xl px-2.5 hover:bg-white/5 text-slate-300 transition duration-100"
            >
              <Activity className="mr-2 h-3.5 w-3.5 text-teal-400" />
              <span className="truncate text-xs">Quét OTP Microsoft</span>
            </Button>
          </div>
        </div>

        <div className="h-px bg-white/5" />

        {/* Section 3: Status Management */}
        <div>
          <p className="px-2 pb-2 text-[10px] font-bold uppercase tracking-[0.2em] text-slate-500 flex items-center gap-1.5">
            <BadgeCheck className="h-3 w-3 text-slate-400" />
            Trạng thái thủ công
          </p>
          <div className="grid grid-cols-2 gap-2">
            <Button
              variant="ghost"
              size="sm"
              onClick={() => {
                onClose();
                onMarkAccountStatus(acc.email, "Success");
              }}
              className="h-9 rounded-xl border border-emerald-500/10 hover:border-emerald-500/30 bg-emerald-500/5 hover:bg-emerald-500/10 text-emerald-400 hover:text-emerald-300 text-xs transition duration-100"
            >
              <BadgeCheck className="mr-1 h-3 w-3" />
              Success
            </Button>

            <Button
              variant="ghost"
              size="sm"
              onClick={() => {
                onClose();
                onMarkAccountStatus(acc.email, "Fail");
              }}
              className="h-9 rounded-xl border border-rose-500/10 hover:border-rose-500/30 bg-rose-500/5 hover:bg-rose-500/10 text-rose-400 hover:text-rose-300 text-xs transition duration-100"
            >
              <Activity className="mr-1 h-3 w-3" />
              Fail
            </Button>

            <Button
              variant="ghost"
              size="sm"
              onClick={() => {
                onClose();
                onMarkAccountStatus(acc.email, "Sold");
              }}
              className="h-9 rounded-xl border border-amber-500/10 hover:border-amber-500/30 bg-amber-500/5 hover:bg-amber-500/10 text-amber-400 hover:text-amber-300 text-xs transition duration-100"
            >
              <Activity className="mr-1 h-3 w-3" />
              Sold
            </Button>

            <Button
              variant="ghost"
              size="sm"
              onClick={() => {
                onClose();
                onMarkAccountStatus(acc.email, "Pending");
              }}
              className="h-9 rounded-xl border border-slate-500/10 hover:border-slate-500/30 bg-slate-500/5 hover:bg-slate-500/10 text-slate-400 hover:text-slate-300 text-xs transition duration-100"
            >
              <LoaderCircle className="mr-1 h-3 w-3 animate-spin" style={{ animationDuration: '3s' }} />
              Reset
            </Button>
          </div>
        </div>

      </div>
    </div>
  );
}
