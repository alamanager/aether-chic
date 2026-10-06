import { memo, useState } from "react";
import { ChevronDown, RefreshCw, ShieldCheck, Users } from "lucide-react";
import { cn } from "@/lib/utils";
import { fmtBytes, useClientUsage, type UserRow } from "@/state/clientUsage";

function fmtDur(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${s % 60}s`;
  return `${s}s`;
}

function fmtClock(ms: number): string {
  return new Date(ms).toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" });
}

const UserDetail = memo(function UserDetail({ row }: { row: UserRow }) {
  return (
    <div className="flex flex-col gap-0.5 px-6 pt-0.5 pb-1.5 text-[11px] text-muted-foreground">
      <div className="flex justify-between">
        <span>Live connections</span>
        <span className="font-mono" dir="ltr">
          {row.conns} ({row.via.join("+")})
        </span>
      </div>
      <div className="flex justify-between">
        <span>Online this visit</span>
        <span className="font-mono" dir="ltr">
          {row.firstSeen === 0 ? "…" : fmtDur(row.totalMs)}
        </span>
      </div>
      <div className="flex justify-between">
        <span>First seen</span>
        <span className="font-mono" dir="ltr">
          {row.firstSeen === 0 ? "…" : fmtClock(row.firstSeen)}
        </span>
      </div>
      {(row.totalUp !== null || row.totalDown !== null) && (
        <div className="flex justify-between">
          <span>Transferred</span>
          <span className="font-mono" dir="ltr">
            ↓{row.totalDown !== null ? fmtBytes(row.totalDown) : "—"} ↑
            {row.totalUp !== null ? fmtBytes(row.totalUp) : "—"}
          </span>
        </div>
      )}
      <div className="flex justify-between gap-2">
        <span className="shrink-0">Device</span>
        <span className="truncate font-mono" dir="ltr">
          {row.name ?? "no name answered"}
        </span>
      </div>
    </div>
  );
});

/**
 * Who is on your proxy: per-user rows (tap to expand), session times,
 * best-effort device names, and — when elevated — transferred bytes.
 * Polls only while focused; rows are memoized so a 10s refresh is cheap.
 */
export function ClientsCard() {
  const { rows, people, metered, elevated, adminHint, spinning, refresh, resolve, enableAdmin } =
    useClientUsage();
  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  const toggle = (ip: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(ip)) next.delete(ip);
      else {
        next.add(ip);
        resolve(ip);
      }
      return next;
    });
  };

  return (
    <div className="flex flex-col gap-1.5 rounded-xl bg-black/25 px-3 py-2 ring-1 ring-white/10 light:bg-black/[0.04] light:ring-black/10">
      <div className="flex h-6 items-center justify-between">
        <span className="flex items-center gap-1.5 text-[10px] font-bold tracking-[0.2em] text-muted-foreground uppercase">
          <Users className="size-3.5" />
          Connected clients
          {people !== null && (
            <span className="rounded-full bg-status-connected/15 px-2 py-0.5 font-mono text-[10px] font-bold text-status-connected">
              {people}
            </span>
          )}
          {metered && (
            <span className="flex items-center gap-0.5 rounded-full bg-status-connected/15 px-2 py-0.5 text-[10px] font-bold text-status-connected">
              <ShieldCheck className="size-3" />
              ADMIN
            </span>
          )}
        </span>
        <button
          type="button"
          onClick={() => void refresh()}
          aria-label="Refresh client counts"
          className="grid size-7 place-items-center rounded-lg text-muted-foreground transition-colors hover:bg-surface-3 hover:text-foreground"
        >
          <RefreshCw className={cn("size-3.5", spinning && "animate-spin")} />
        </button>
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
          Nobody yet — share the SOCKS or HTTP address above.
        </p>
      ) : (
        rows.map((row) => {
          const open = expanded.has(row.ip);
          return (
            <div key={row.ip} className="flex flex-col rounded-lg">
              <button
                type="button"
                onClick={() => toggle(row.ip)}
                aria-expanded={open}
                className="flex items-center gap-2 rounded-lg px-1 py-1 text-left transition-colors hover:bg-surface-3"
              >
                <ChevronDown
                  className={cn("size-3.5 shrink-0 text-muted-foreground transition-transform", open && "rotate-180")}
                />
                <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-foreground" dir="ltr">
                  {row.name ?? row.ip}
                </span>
                <span className="shrink-0 font-mono text-[10px] text-muted-foreground" dir="ltr">
                  ×{row.conns}
                  {row.rateDown !== null && row.rateDown > 0 ? ` · ↓${fmtBytes(row.rateDown)}/s` : ""}
                  {row.totalMs > 0 ? ` · ${fmtDur(row.totalMs)}` : ""}
                </span>
              </button>
              {open && <UserDetail row={row} />}
            </div>
          );
        })
      )}
    </div>
  );
}
