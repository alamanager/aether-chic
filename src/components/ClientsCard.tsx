import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ChevronDown, RefreshCw, Users } from "lucide-react";
import { cn } from "@/lib/utils";
import { useWindowFocused } from "@/state/windowFocus";

interface ClientEntry {
  ip: string;
  connections: number;
}

interface EndpointClients {
  label: string;
  port: number;
  clients: ClientEntry[];
  local_connections: number;
  total_connections: number;
}

interface Session {
  firstSeen: number;
  totalMs: number;
}

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

/**
 * Who is on your proxy right now: ESTABLISHED connections per endpoint,
 * grouped by remote IP (loopback = this machine, counted separately).
 *
 * Tap a user to expand: live connections across endpoints, session time
 * this visit, first seen, and a best-effort device name (reverse DNS —
 * phones and most home routers answer nothing, then it stays the IP).
 *
 * Perf: polls only while the window is focused; lookups run once per IP
 * and are cached; session times accumulate locally, no backend timer.
 */
export function ClientsCard() {
  const [data, setData] = useState<EndpointClients[] | null>(null);
  const [spinning, setSpinning] = useState(false);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [sessions, setSessions] = useState<Record<string, Session>>({});
  const [names, setNames] = useState<Record<string, string | null>>({});
  const focused = useWindowFocused();
  const lastPoll = useRef<number>(0);

  const refresh = useCallback(async () => {
    setSpinning(true);
    try {
      const r = await invoke<EndpointClients[]>("proxy_clients");
      setData(r);
      const now = Date.now();
      const dt = lastPoll.current === 0 ? 0 : Math.min(now - lastPoll.current, 30000);
      lastPoll.current = now;
      if (dt > 0) {
        const seen = new Set<string>();
        r.forEach((e) => e.clients.forEach((c) => seen.add(c.ip)));
        setSessions((prev) => {
          const next: Record<string, Session> = { ...prev };
          seen.forEach((ip) => {
            const old = next[ip];
            next[ip] = old
              ? { firstSeen: old.firstSeen, totalMs: old.totalMs + dt }
              : { firstSeen: now, totalMs: 0 };
          });
          return next;
        });
      } else {
        const nowSeen = new Set<string>();
        r.forEach((e) => e.clients.forEach((c) => nowSeen.add(c.ip)));
        setSessions((prev) => {
          const next: Record<string, Session> = { ...prev };
          nowSeen.forEach((ip) => {
            if (!next[ip]) next[ip] = { firstSeen: now, totalMs: 0 };
          });
          return next;
        });
      }
    } catch {
      /* backend without the command (older backend): stay silent */
    } finally {
      setSpinning(false);
    }
  }, []);

  /* eslint-disable react-hooks/set-state-in-effect -- initial fetch on
   * mount; polling afterwards. Same pattern as ConnectionInfo. */
  useEffect(() => {
    lastPoll.current = 0;
    void refresh();
    if (!focused) return;
    const id = setInterval(() => void refresh(), 10000);
    return () => clearInterval(id);
  }, [refresh, focused]);
  /* eslint-enable react-hooks/set-state-in-effect */

  const toggle = useCallback(
    (ip: string) => {
      setExpanded((prev) => {
        const next = new Set(prev);
        if (next.has(ip)) next.delete(ip);
        else {
          next.add(ip);
          if (!(ip in names)) {
            invoke<string | null>("resolve_host", { ip })
              .then((n) => setNames((m) => ({ ...m, [ip]: n })))
              .catch(() => setNames((m) => ({ ...m, [ip]: null })));
          }
        }
        return next;
      });
    },
    [names],
  );

  const people = data?.reduce((n, e) => n + e.clients.length, 0) ?? null;
  // One row per person across endpoints (same IP on SOCKS+HTTP = one user).
  const byIp = new Map<string, { connections: number; via: string[] }>();
  data?.forEach((e) =>
    e.clients.forEach((c) => {
      const cur = byIp.get(c.ip) ?? { connections: 0, via: [] };
      cur.connections += c.connections;
      if (!cur.via.includes(e.label)) cur.via.push(e.label);
      byIp.set(c.ip, cur);
    }),
  );
  const rows = [...byIp.entries()].sort((a, b) => b[1].connections - a[1].connections);

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
      {data === null ? (
        <p className="px-1 pb-1 font-mono text-[11px] text-muted-foreground" dir="ltr">
          …
        </p>
      ) : rows.length === 0 ? (
        <p className="px-1 pb-1 text-[11px] text-muted-foreground">
          Nobody yet — share the SOCKS or HTTP address above.
        </p>
      ) : (
        rows.slice(0, 12).map(([ip, info]) => {
          const open = expanded.has(ip);
          const sess = sessions[ip];
          const name = names[ip];
          return (
            <div key={ip} className="flex flex-col rounded-lg">
              <button
                type="button"
                onClick={() => toggle(ip)}
                aria-expanded={open}
                className="flex items-center gap-2 rounded-lg px-1 py-1 text-left transition-colors hover:bg-surface-3"
              >
                <ChevronDown
                  className={cn("size-3.5 shrink-0 text-muted-foreground transition-transform", open && "rotate-180")}
                />
                <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-foreground" dir="ltr">
                  {name || ip}
                </span>
                <span className="shrink-0 font-mono text-[10px] text-muted-foreground" dir="ltr">
                  ×{info.connections} · {sess ? fmtDur(sess.totalMs) : "…"}
                </span>
              </button>
              {open && (
                <div className="flex flex-col gap-0.5 px-6 pt-0.5 pb-1.5 text-[11px] text-muted-foreground">
                  <div className="flex justify-between">
                    <span>Live connections</span>
                    <span className="font-mono" dir="ltr">
                      {info.connections} ({info.via.join("+")})
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span>Online this visit</span>
                    <span className="font-mono" dir="ltr">
                      {sess ? fmtDur(sess.totalMs) : "…"}
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span>First seen</span>
                    <span className="font-mono" dir="ltr">
                      {sess ? fmtClock(sess.firstSeen) : "…"}
                    </span>
                  </div>
                  <div className="flex justify-between gap-2">
                    <span className="shrink-0">Device</span>
                    <span className="truncate font-mono" dir="ltr">
                      {ip in names ? (name ?? "no name answered") : "looking up…"}
                    </span>
                  </div>
                </div>
              )}
            </div>
          );
        })
      )}
    </div>
  );
}
