import type { ReactNode } from "react";

export function Tag({ icon, children }: { icon?: ReactNode; children: ReactNode }) {
  return (
    <div className="flex h-7 items-center gap-2 self-start rounded-full bg-raised px-3 text-xs font-medium text-dim [&_svg]:h-3.5 [&_svg]:w-3.5 [&_svg]:text-fg">
      {icon}
      {children}
    </div>
  );
}

export function Heading({ light, bold, size = 42 }: { light: string; bold: string; size?: 28 | 42 | 56 }) {
  const cls = size === 56 ? "text-[56px]" : size === 42 ? "text-[42px]" : "text-[28px]";
  const Tag = size === 28 ? "h2" : "h1";
  return (
    <Tag className={cls}>
      <span className="font-light">{light}</span> <span className="font-bold">{bold}</span>
    </Tag>
  );
}

export function PageHeader({
  tag,
  light,
  bold,
  note,
  actions,
}: {
  tag?: ReactNode;
  light: string;
  bold: string;
  note?: string;
  actions?: ReactNode;
}) {
  return (
    <div className="flex items-end gap-3">
      <div className="flex flex-1 flex-col gap-3.5">
        {tag}
        <Heading light={light} bold={bold} />
        {note && <p className="m-0 max-w-[60ch] text-[15px] leading-[1.55] text-dim">{note}</p>}
      </div>
      {actions}
    </div>
  );
}
