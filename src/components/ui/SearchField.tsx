import { Search } from "lucide-react";
import { forwardRef, type InputHTMLAttributes } from "react";

type Props = InputHTMLAttributes<HTMLInputElement> & { size?: 36 | 44; hotkey?: string; active?: boolean };

export const SearchField = forwardRef<HTMLInputElement, Props>(function SearchField(
  { size = 44, hotkey, active = false, className = "", ...rest },
  ref,
) {
  return (
    <label
      className={`flex items-center gap-2.5 rounded-full bg-raised px-3.5 text-dim ${size === 36 ? "h-9 text-[13px]" : "h-11 text-sm"} ${
        active ? "outline-[1.5px] outline-fg outline-solid" : ""
      } ${className}`}
    >
      <Search className="h-4 w-4 shrink-0" strokeWidth={1.5} />
      <input
        ref={ref}
        className="min-w-0 flex-1 bg-transparent font-medium text-fg outline-none placeholder:font-normal placeholder:text-dim"
        {...rest}
      />
      {hotkey && <span className="font-medium">{hotkey}</span>}
    </label>
  );
});
