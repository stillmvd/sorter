import type { ButtonHTMLAttributes, ReactNode } from "react";
import { Kbd } from "./Kbd";

type Props = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "secondary" | "ghost";
  size?: 36 | 40 | 44 | 48;
  hotkey?: string;
  icon?: ReactNode;
};

const heights = { 36: "h-9 px-3.5 text-[13px]", 40: "h-10 px-4 text-sm", 44: "h-11 px-[18px] text-sm", 48: "h-12 px-6 text-[15px]" };

export function Button({ variant = "secondary", size = 40, hotkey, icon, className = "", children, ...rest }: Props) {
  const look =
    variant === "primary"
      ? "bg-fg text-ink font-bold hover:bg-[color-mix(in_oklab,var(--fg)_86%,var(--ground))] disabled:bg-strong disabled:text-dim"
      : variant === "ghost"
        ? "bg-transparent text-fg font-medium hover:bg-raised"
        : "bg-raised text-fg font-medium hover:bg-strong disabled:text-dim";
  return (
    <button
      type="button"
      className={`inline-flex shrink-0 items-center justify-center gap-2 whitespace-nowrap rounded-full transition-[background-color,transform] duration-200 ease-trail ${heights[size]} ${look} ${className}`}
      {...rest}
    >
      {icon}
      {children}
      {hotkey && <Kbd inverted={variant === "primary"}>{hotkey}</Kbd>}
    </button>
  );
}
