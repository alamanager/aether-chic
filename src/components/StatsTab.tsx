import { ShieldCheck } from "lucide-react";
import { SpeedCard } from "@/components/SpeedCard";
import { TransferGraph } from "@/components/TransferGraph";
import { fmtBytes, useClientUsage } from "@/state/clientUsage";

/** Stats tab: tunnel throughput + flow history + per-user usage table. */
export function StatsTab() {
  const { rows, people, metered, elevated, adminHint, enableAdmin, resolve } = useClientUsage();

  return (
    <div className="flex w-full flex-col items-center gap-3">
      <SpeedCard />
      <TransferGraph />
      <div className="glass flex w-full max-w-sm flex-col gap-1.5 rounded-2xl p-3">
        <div className="flex h-6 items-center justify-between px-1">
          <span className="text-[10px] font-bold tracking-[0.2em] text-muted-foreground uppercase">
            Per-user usage
            {people !== null && people > 0 && (
              <span className="ml-2 rounded-full bg-status-connected/15 px-2 py-0.5 font-mono text-[10px] font-bold text-status-connected">
                {people}
              </span>
            )}
          </span>
          {metered && (
            <span className="flex items-center gap-0.5 rounded-full bg-status-connected/15 px-2 py-0.5 text-[10px] font-bold text-status-connected">
              <ShieldCheck className="size-3" />
              ADMIN
            </span>
          )}
        </div>
        {!metered && elevated === false && (
          <button
            type="button"
            onClick={() => void enableAdmin()}
            className="rounded-lg bg-surface-3 px-2 py-1.5 text-[11px] font-semibold text-foreground transition-colors hover:bg-surface-4"
          >
            Enable per-user bytes (admin)
          </button>
        )}
        {adminHint && <p className="px-1 text-[11px] text-muted-foreground">{adminHint}</p>}
        {rows.length === 0 ? (
          <p className="px-1 pb-1 text-[11px] text-muted-foreground">
            No LAN clients this session. Byte columns need admin — without it
            you still get live connections and session times.
          </p>
        ) : (
          <div className="flex flex-col">
            <div className="flex items-center justify-between px-1 font-mono text-[10px] text-muted-foreground" dir="ltr">
              <span className="flex-1">user</span>
              <span className="w-14 text-right">down</span>
              <span className="w-14 text-right">up</span>
              <span className="w-16 text-right">rate</span>
            </div>
            {rows.map((r) => (
              <button
                key={r.ip}
                type="button"
                onClick={() => resolve(r.ip)}
                title={r.name ?? r.ip}
                className="flex items-center justify-between gap-2 rounded-lg px-1 py-1.5 text-left font-mono text-[11px] transition-colors hover:bg-surface-3"
              >
                <span className="min-w-0 flex-1 truncate text-foreground" dir="ltr">
                  {r.name ?? r.ip}
                </span>
                <span className="w-14 shrink-0 text-right text-muted-foreground" dir="ltr">
                  {r.totalDown !== null ? fmtBytes(r.totalDown) : "—"}
                </span>
                <span className="w-14 shrink-0 text-right text-muted-foreground" dir="ltr">
                  {r.totalUp !== null ? fmtBytes(r.totalUp) : "—"}
                </span>
                <span className="w-16 shrink-0 text-right text-status-connected" dir="ltr">
                  {r.rateDown !== null && r.rateDown > 0 ? `↓${fmtBytes(r.rateDown)}/s` : "—"}
                </span>
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
