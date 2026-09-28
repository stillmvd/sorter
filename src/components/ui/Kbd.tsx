import type { ReactNode } from "react";

export function Kbd({ children, inverted = false }: { children: ReactNode; inverted?: boolean }) {
  return inverted ? (
    <span className="grid h-[22px] place-items-center rounded-full border border-ink px-2 text-[11px] font-bold">
      {children}
    </span>
  ) : (
    <span className="text-xs font-medium text-dim">{children}</span>
  );
}
