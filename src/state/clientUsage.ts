import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface ClientEntry {
  ip: string;
  connections: number;
  conn_id: string;
  bytes_up: number | null;
  bytes_down: number | null;
}

export interface EndpointClients {
  label: string;
  port: number;
  clients: ClientEntry[];
  local_connections: number;
  total_connections: number;
  metered: boolean;
}

export interface UserRow {
  ip: string;
  name: string | null;
  conns: number;
  via: string[];
  totalUp: number | null;
  totalDown: number | null;
  rateUp: number | null;
  rateDown: number | null;
  firstSeen: number;
  totalMs: number;
}

export function fmtBytes(b: number): string {
  if (b < 1024) return `${Math.round(b)} B`;
  if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`;
  if (b < 1024 * 1024 * 1024) return `${(b / (1024 * 1024)).toFixed(1)} MB`;
  return `${(b / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

export function fmtDur(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${s % 60}s`;
  return `${s}s`;
}

export function fmtClock(ms: number): string {
  return new Date(ms).toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" });
}

interface Session {
  firstSeen: number;
  totalMs: number;
}

/**
 * Shared per-user usage: polls proxy_clients, accumulates session time by
 * presence and byte totals by connection-id deltas (survives 5-tuple
 * churn), resolves device names on demand (cached).
 *
 * All cross-poll memory lives at MODULE scope, not in component state:
 * tab switches unmount/remount consumers, and state would wipe first-seen
 * times and byte totals on every visit. One consumer is mounted at a time
 * (Pulse footer XOR Stats tab), so a single shared cache is correct.
 *
 * Polling is NOT gated on window focus: the focus signal has known
 * false-negatives on Windows (WebView2 child focus) that silently froze
 * updates while staring at the app. One 10s poll is negligible.
 */
const cache = {
  sessions: {} as Record<string, Session>,
  totals: {} as Record<string, { up: number; down: number }>,
  rates: {} as Record<string, { up: number; down: number }>,
  names: {} as Record<string, string | null>,
  prevConns: new Map<string, { up: number; down: number }>(),
  requested: new Set<string>(),
  lastPoll: 0,
};

export function useClientUsage() {
  const [data, setData] = useState<EndpointClients[] | null>(null);
  const [spinning, setSpinning] = useState(false);
  // Initialized from the module cache so a tab revisit keeps history.
  const [sessions, setSessions] = useState<Record<string, Session>>(() => ({ ...cache.sessions }));
  const [totals, setTotals] = useState<Record<string, { up: number; down: number }>>(() => ({ ...cache.totals }));
  const [rates, setRates] = useState<Record<string, { up: number; down: number }>>(() => ({ ...cache.rates }));
  const [names, setNames] = useState<Record<string, string | null>>(() => ({ ...cache.names }));
  const [elevated, setElevated] = useState<boolean | null>(null);
  const [adminHint, setAdminHint] = useState<string | null>(null);

  useEffect(() => {
    invoke<boolean>("is_elevated")
      .then(setElevated)
      .catch(() => setElevated(false));
  }, []);

  const refresh = useCallback(async () => {
    setSpinning(true);
    try {
      const r = await invoke<EndpointClients[]>("proxy_clients");
      setData(r);
      const now = Date.now();
      const dtMs = cache.lastPoll === 0 ? 0 : now - cache.lastPoll;
      cache.lastPoll = now;
      const dt = Math.min(dtMs, 30000);
      const seen = new Set<string>();
      r.forEach((e) => e.clients.forEach((c) => seen.add(c.ip)));
      const ns: Record<string, Session> = { ...cache.sessions };
      seen.forEach((ip) => {
        const old = ns[ip];
        ns[ip] = old
          ? { firstSeen: old.firstSeen, totalMs: old.totalMs + (dt > 0 ? dt : 0) }
          : { firstSeen: now, totalMs: 0 };
      });
      cache.sessions = ns;
      setSessions(ns);
      if (dt > 0) {
        const dtS = dt / 1000;
        const nt: Record<string, { up: number; down: number }> = {};
        const nr: Record<string, { up: number; down: number }> = {};
        r.forEach((e) =>
          e.clients.forEach((c) => {
            if (c.bytes_up === null || c.bytes_down === null) return;
            const prev = cache.prevConns.get(c.conn_id);
            const cur = { up: c.bytes_up, down: c.bytes_down };
            cache.prevConns.set(c.conn_id, cur);
            if (!prev) return;
            const du = c.bytes_up >= prev.up ? c.bytes_up - prev.up : c.bytes_up;
            const dd = c.bytes_down >= prev.down ? c.bytes_down - prev.down : c.bytes_down;
            if (du === 0 && dd === 0) return;
            const t = nt[c.ip] ?? { up: 0, down: 0 };
            t.up += du;
            t.down += dd;
            nt[c.ip] = t;
            nr[c.ip] = { up: du / dtS, down: dd / dtS };
          }),
        );
        if (Object.keys(nt).length > 0) {
          const merged = { ...cache.totals };
          Object.entries(nt).forEach(([ip, d]) => {
            const o = merged[ip] ?? { up: 0, down: 0 };
            merged[ip] = { up: o.up + d.up, down: o.down + d.down };
          });
          cache.totals = merged;
          setTotals(merged);
          cache.rates = nr;
          setRates(nr);
        } else {
          cache.rates = {};
          setRates({});
        }
      }
    } catch {
      /* older backend: stay silent */
    } finally {
      setSpinning(false);
    }
  }, []);

  /* eslint-disable react-hooks/set-state-in-effect -- mount fetch + poll.
   * Deliberately NOT gated on window focus (see module doc). */
  useEffect(() => {
    cache.lastPoll = 0;
    void refresh();
    const id = setInterval(() => void refresh(), 10000);
    return () => clearInterval(id);
  }, [refresh]);
  /* eslint-enable react-hooks/set-state-in-effect */

  const resolve = useCallback((ip: string) => {
    if (cache.requested.has(ip)) return;
    cache.requested.add(ip);
    invoke<string | null>("resolve_host", { ip })
      .then((n) => {
        cache.names = { ...cache.names, [ip]: n };
        setNames(cache.names);
      })
      .catch(() => {
        cache.names = { ...cache.names, [ip]: null };
        setNames(cache.names);
      });
  }, []);

  const enableAdmin = useCallback(async (): Promise<boolean> => {
    try {
      const ok = await invoke<boolean>("request_admin");
      if (!ok) {
        setAdminHint("Admin declined — per-user bytes stay off.");
        return false;
      }
      setAdminHint(null);
      const el = await invoke<boolean>("is_elevated").catch(() => false);
      setElevated(el);
      return el;
    } catch {
      setAdminHint("Could not request admin.");
      return false;
    }
  }, []);

  const rows: UserRow[] = (() => {
    const byIp = new Map<string, { conns: number; via: string[] }>();
    data?.forEach((e) =>
      e.clients.forEach((c) => {
        const cur = byIp.get(c.ip) ?? { conns: 0, via: [] as string[] };
        cur.conns += c.connections;
        if (!cur.via.includes(e.label)) cur.via.push(e.label);
        byIp.set(c.ip, cur);
      }),
    );
    return [...byIp.entries()]
      .map(([ip, info]) => {
        const t = totals[ip];
        const r = rates[ip];
        const s = sessions[ip];
        return {
          ip,
          name: names[ip] ?? null,
          conns: info.conns,
          via: info.via,
          totalUp: t?.up ?? null,
          totalDown: t?.down ?? null,
          rateUp: r?.up ?? null,
          rateDown: r?.down ?? null,
          firstSeen: s?.firstSeen ?? 0,
          totalMs: s?.totalMs ?? 0,
        };
      })
      .sort((a, b) => b.conns - a.conns)
      .slice(0, 12);
  })();

  return {
    data,
    rows,
    people: data?.reduce((n, e) => n + e.clients.length, 0) ?? null,
    metered: data?.some((e) => e.metered) ?? false,
    elevated,
    adminHint,
    spinning,
    refresh,
    resolve,
    enableAdmin,
  };
}
