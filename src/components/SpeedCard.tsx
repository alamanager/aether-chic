import { ArrowDownToLine, ArrowUpFromLine } from "lucide-react";
import { useConnectionStore } from "@/state/connectionStore";
import { cn } from "@/lib/utils";

function fmt(bytes: number): string {
  if (bytes < 1024) return `${Math.round(bytes)} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

export function SpeedCard() {
  const traffic = useConnectionStore((s) => s.traffic);
  const connected = useConnectionStore((s) => s.status.state === "Connected");

  const row = (icon: React.ReactNode, label: string, rate: string, total: string, tint: string) => (
    <div className="flex min-h-[68px] flex-1 items-center gap-2.5 overflow-hidden rounded-xl bg-black/25 px-3 py-2 ring-1 ring-white/10 light:bg-black/[0.04] light:ring-black/10">
      <span className={cn("shrink-0", tint)}>{icon}</span>
      <span className="min-w-0 flex-1 text-left">
        <span className="block text-[10px] font-semibold tracking-widest text-muted-foreground uppercase">
          {label}
        </span>
        <span className="block truncate font-mono text-sm font-bold text-foreground" dir="ltr">
          {rate}
        </span>
        <span className="block truncate font-mono text-[10px] text-muted-foreground" dir="ltr">
          Σ {total}
        </span>
      </span>
    </div>
  );

  return (
    <div className={cn("glass flex w-full max-w-sm flex-col gap-2 rounded-2xl p-3 transition-opacity", !connected && "opacity-70")}>
      <div className="flex h-6 items-center justify-between px-1">
        <span className="text-[10px] font-bold tracking-[0.2em] text-muted-foreground uppercase">
          Throughput
        </span>
        <span className="flex items-center gap-1.5 font-mono text-[10px] text-muted-foreground" dir="ltr">
          <span className={cn("size-1.5 rounded-full", traffic ? "anim-glow-fast bg-status-connected" : "bg-status-idle")} aria-hidden />
          {traffic ? "live" : "idle"}
        </span>
      </div>
      <div className="flex items-center gap-2">
        {row(<ArrowDownToLine className="size-4" />, "Down", traffic ? `${fmt(traffic.downRate)}/s` : "—", traffic ? fmt(traffic.down) : "—", "text-cyan-300")}
        {row(<ArrowUpFromLine className="size-4" />, "Up", traffic ? `${fmt(traffic.upRate)}/s` : "—", traffic ? fmt(traffic.up) : "—", "text-violet-300")}
      </div>
    </div>
  );
}
