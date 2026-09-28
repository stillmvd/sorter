import type { ButtonHTMLAttributes, ReactNode } from "react";

type Props = ButtonHTMLAttributes<HTMLButtonElement> & { label: string; hint?: string; children: ReactNode };

export function IconButton({ label, hint, children, className = "", ...rest }: Props) {
  return (
    <button
      type="button"
      aria-label={label}
      title={hint ?? label}
      className={`grid h-10 w-10 shrink-0 place-items-center rounded-full bg-raised text-fg transition-colors duration-200 ease-trail hover:bg-strong ${className}`}
      {...rest}
    >
      {children}
    </button>
  );
}
