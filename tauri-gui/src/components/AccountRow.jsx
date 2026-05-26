import { Play, MoreHorizontal, LoaderCircle } from "lucide-react";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { AccountActionDropdown } from "./AccountActionDropdown";
import { cn } from "../lib/utils";

export function AccountRow({
  acc,
  index,
  nextStep,
  nextStepInfo,
  isRowRunning,
  isSelected,
  isMenuOpen,
  visibleBadges,
  status,
  getBadgeVariant,
  handleStartAutomation,
  toggleActionMenu,
  closeActionMenu,
  handleCopyToken,
  toggleSelectEmail,
  triggerGetOTP,
  markAccountStatus,
}) {
  return (
    <tr 
      className={cn(
        "group relative border-b border-white/5 align-top transition",
        isMenuOpen ? "bg-white/[0.04] z-20 relative" : "hover:bg-white/[0.03] z-0"
      )}
    >
      <td className="px-6 py-5 text-center">
        <button
          onClick={() => toggleSelectEmail(acc.email)}
          className={cn(
            "mx-auto flex h-8 w-8 items-center justify-center rounded-full border transition",
            isSelected
              ? "border-amber-200 bg-amber-300 text-slate-950"
              : "border-white/10 bg-white/5 text-slate-400 hover:border-amber-300 hover:text-white",
          )}
        >
          <span className={cn("block h-3 w-3 rounded-full", isSelected ? "bg-slate-950" : "border border-current")} />
        </button>
      </td>
      
      <td className="px-6 py-5">
        <div className="space-y-3">
          <div className="flex flex-col gap-3 xl:flex-row xl:items-start xl:justify-between">
            <div className="min-w-0">
              <p className="truncate text-[15px] font-semibold tracking-tight text-white">{acc.email}</p>
              <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-slate-400">
                <span>{acc.created}</span>
                {acc.accountId && <span>ID {acc.accountId}</span>}
                <span>{acc.token ? "Token ready" : "No token"}</span>
                {acc.paypalLink && (
                  <button
                    type="button"
                    title="Click để copy PayPal link"
                    className="text-amber-300 hover:text-amber-200"
                    onClick={async (e) => {
                      e.stopPropagation();
                      await navigator.clipboard.writeText(acc.paypalLink);
                    }}
                  >
                    PayPal ready
                  </button>
                )}
              </div>
            </div>
            <Badge variant="secondary" className="w-fit rounded-full px-3 py-1 text-[10px] tracking-[0.18em]">
              {nextStepInfo?.shortLabel || nextStep}
            </Badge>
          </div>

          {visibleBadges.length > 0 && (
            <div className="flex flex-wrap gap-1.5">
              {visibleBadges.map((badge, badgeIndex) => {
                if (badge === "Login OK") {
                  return (
                    <Button
                      key={`${acc.email}-${badgeIndex}`}
                      size="sm"
                      variant="success"
                      title="Click để copy Access Token"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleCopyToken(acc.token, acc.email);
                      }}
                      className="h-auto rounded-full px-3 py-1 text-[10px] uppercase tracking-[0.16em]"
                    >
                      {badge}
                    </Button>
                  );
                }

                return (
                  <Badge
                    key={`${acc.email}-${badgeIndex}`}
                    variant={getBadgeVariant(badge)}
                    className="rounded-full px-3 py-1 text-[10px] uppercase tracking-[0.14em]"
                  >
                    {badge}
                  </Badge>
                );
              })}
            </div>
          )}
        </div>
      </td>
      
      <td className="px-6 py-5 overflow-visible">
        <div className="flex items-center justify-end gap-2 relative">
          <Button
            size="sm"
            onClick={() => handleStartAutomation(acc.email, nextStep)}
            disabled={status === "running"}
            className="min-w-[112px] rounded-xl"
          >
            {isRowRunning ? (
              <>
                <LoaderCircle className="h-4 w-4 animate-spin" />
                Running
              </>
            ) : (
              <>
                <Play className="h-4 w-4" />
                Run
              </>
            )}
          </Button>
          
          <div className="relative">
            <Button
              variant={isMenuOpen ? "secondary" : "outline"}
              size="icon"
              onClick={(e) => {
                e.stopPropagation();
                toggleActionMenu(acc.email);
              }}
              className="h-10 w-10 rounded-xl border-white/10 bg-white/5 text-slate-200 hover:bg-white/10"
            >
              <MoreHorizontal className="h-4 w-4" />
            </Button>
            
            <AccountActionDropdown
              acc={acc}
              isOpen={isMenuOpen}
              onClose={closeActionMenu}
              nextStep={nextStep}
              onStartAutomation={handleStartAutomation}
              onTriggerGetOTP={triggerGetOTP}
              onMarkAccountStatus={markAccountStatus}
            />
          </div>
        </div>
      </td>
    </tr>
  );
}
