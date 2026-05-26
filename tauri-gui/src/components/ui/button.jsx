import * as React from "react";
import { cva } from "class-variance-authority";
import { cn } from "../../lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-xl text-sm font-medium transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-300/60 disabled:pointer-events-none disabled:opacity-50",
  {
    variants: {
      variant: {
        default:
          "bg-amber-300 text-slate-950 shadow-[0_10px_30px_-14px_rgba(252,211,77,0.75)] hover:bg-amber-200",
        secondary:
          "bg-slate-900/80 text-slate-100 border border-white/10 hover:bg-slate-800/80",
        outline:
          "border border-white/10 bg-slate-950/40 text-slate-200 hover:bg-slate-900/80 hover:text-white",
        ghost: "text-slate-300 hover:bg-white/5 hover:text-white",
        success:
          "border border-emerald-500/20 bg-emerald-500/10 text-emerald-200 hover:bg-emerald-500/20",
        destructive:
          "border border-rose-500/20 bg-rose-500/10 text-rose-200 hover:bg-rose-500/20",
      },
      size: {
        default: "h-10 px-4 py-2",
        sm: "h-8 rounded-lg px-3 text-xs",
        lg: "h-11 rounded-xl px-6",
        icon: "h-10 w-10 rounded-xl",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

const Button = React.forwardRef(({ className, variant, size, ...props }, ref) => {
  return <button ref={ref} className={cn(buttonVariants({ variant, size }), className)} {...props} />;
});

Button.displayName = "Button";

export { Button, buttonVariants };
