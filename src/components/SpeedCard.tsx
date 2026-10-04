import { ArrowDownToLine, ArrowUpFromLine } from "lucide-react";
import { useConnectionStore } from "@/state/connectionStore";
import { cn } from "@/lib/utils";

function fmt(bytes: number): string {
  if (bytes < 1024) return `${Math.round(bytes)} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

/**
 * Fixed-size transfer card: live rates + session totals. Always mounted
 * (same box in every state) — dashes until the first core stats line.
 */
export function SpeedCard() {
  const traffic = useConnectionStore((s) => s.traffic);
  const connected = useConnectionStore((s) => s.status.state === "Connected");

  const row = (
    icon: React.ReactNode,
    label: string,
    rate: string,
    total: string,
  ) => (
    <div className="flex min-h-[52px] flex-1 items-center gap-2.5 rounded-xl px-3 py-2 ring-1 transition-opacity bg-black/20 ring-white/10 light:bg-black/5 light:ring-black/10">
      <span className="text-muted-foreground">{icon}</span>
      <span className="min-w-0 flex-1 text-left">
        <span className="block text-[10px] font-medium tracking-wide text-muted-foreground uppercase">
          {label}
        </span>
        <span className="block truncate font-mono text-xs text-foreground" dir="ltr">
          {rate} <span className="opacity-60">· Σ {total}</span>
        </span>
      </span>
    </div>
  );

  return (
    <div
      className={cn(
        "glass flex w-full max-w-sm flex-col gap-2 rounded-2xl p-3 transition-opacity",
        !connected && "opacity-70",
      )}
    >
      <div className="flex h-6 items-center justify-between px-1">
        <span className="text-[10px] font-semibold tracking-widest text-muted-foreground uppercase">
          Transfer
        </span>
        <span className="font-mono text-[10px] text-muted-foreground" dir="ltr">
          {traffic ? "live" : "—"}
        </span>
      </div>
      <div className="flex items-center gap-2">
        {row(
          <ArrowDownToLine className="size-4" />,
          "Download",
          traffic ? `${fmt(traffic.downRate)}/s` : "—",
          traffic ? fmt(traffic.down) : "—",
        )}
        {row(
          <ArrowUpFromLine className="size-4" />,
          "Upload",
          traffic ? `${fmt(traffic.upRate)}/s` : "—",
          traffic ? fmt(traffic.up) : "—",
        )}
      </div>
    </div>
  );
}
