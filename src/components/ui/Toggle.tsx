export function Toggle({ label, checked, onChange }: { label: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      onClick={() => onChange(!checked)}
      className={`flex h-7 w-12 shrink-0 items-center rounded-full p-[3px] transition-colors duration-200 ease-trail ${
        checked ? "justify-end bg-fg" : "justify-start bg-strong"
      }`}
    >
      <span className={`h-[22px] w-[22px] rounded-full ${checked ? "bg-ink" : "bg-dim"}`} />
    </button>
  );
}
