export function Kbd({ children, inverted = false }: { children: string; inverted?: boolean }) {
  const cap = inverted
    ? "border-[color-mix(in_oklab,var(--ink)_35%,transparent)]"
    : "border-line bg-ground text-fg";
  return (
    <span className="inline-flex gap-1">
      {children.split(" ").map((k) => (
        <span
          key={k}
          className={`inline-grid h-[22px] min-w-[22px] place-items-center whitespace-nowrap rounded-md border border-b-2 px-1.5 text-[11px] font-bold leading-none ${cap}`}
        >
          {k}
        </span>
      ))}
    </span>
  );
}
