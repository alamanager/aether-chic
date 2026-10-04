import { Switch } from "@/components/ui/switch";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const INPUT =
  "h-8 w-full rounded-md bg-black/20 px-2 text-xs text-foreground ring-1 ring-white/10 outline-none placeholder:text-muted-foreground/60 focus:ring-primary disabled:opacity-50 light:bg-black/5 light:ring-black/10";

/** Single-line option input. Locked mid-session like every profile control. */
export function TextOpt({
  value,
  onChange,
  disabled,
  placeholder,
  mono,
  label,
}: {
  value: string;
  onChange: (v: string) => void;
  disabled: boolean;
  placeholder?: string;
  mono?: boolean;
  label: string;
}) {
  return (
    <input
      type="text"
      value={value}
      disabled={disabled}
      spellCheck={false}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      dir="ltr"
      aria-label={label}
      className={`${INPUT} ${mono ? "font-mono text-left" : ""}`}
    />
  );
}

/** Multi-line option input (one item per line). */
export function AreaOpt({
  value,
  onChange,
  disabled,
  placeholder,
  label,
}: {
  value: string;
  onChange: (v: string) => void;
  disabled: boolean;
  placeholder?: string;
  label: string;
}) {
  return (
    <textarea
      value={value}
      disabled={disabled}
      spellCheck={false}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      dir="ltr"
      rows={3}
      aria-label={label}
      className="min-h-16 w-full resize-y rounded-md bg-black/20 px-2 py-1.5 font-mono text-xs text-foreground ring-1 ring-white/10 outline-none placeholder:text-muted-foreground/60 focus:ring-primary disabled:opacity-50 light:bg-black/5 light:ring-black/10"
    />
  );
}

/** Labelled switch row. */
export function SwitchRow({
  checked,
  onChange,
  disabled,
  label,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  disabled: boolean;
  label: string;
}) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-xs text-muted-foreground">{label}</span>
      <Switch checked={checked} onCheckedChange={onChange} disabled={disabled} aria-label={label} />
    </div>
  );
}

/** Small dropdown for fixed option sets. */
export function SelectOpt({
  value,
  onChange,
  disabled,
  options,
  label,
}: {
  value: string;
  onChange: (v: string) => void;
  disabled: boolean;
  options: [string, string][];
  label: string;
}) {
  return (
    <Select value={value} onValueChange={onChange} disabled={disabled}>
      <SelectTrigger
        size="sm"
        className="w-full border-transparent bg-transparent text-muted-foreground shadow-none hover:bg-surface-2"
        aria-label={label}
      >
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {options.map(([v, l]) => (
          <SelectItem key={v} value={v}>
            {l}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
