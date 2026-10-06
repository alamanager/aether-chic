import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useWindowFocused } from "@/state/windowFocus";

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

interface Session {
  firstSeen: number;
  totalMs: number;
}

/**
 * Shared per-user usage: polls proxy_clients while focused, accumulates
 * session time by presence and byte totals by connection-id deltas
 * (survives 5-tuple churn), resolves device names on demand (cached).
 * One poller per mounted consumer; tabs never co-mount.
 */
export function useClientUsage() {
  const [data, setData] = useState<EndpointClients[] | null>(null);
  const [spinning, setSpinning] = useState(false);
  const [sessions, setSessions] = useState<Record<string, Session>>({});
  const [totals, setTotals] = useState<Record<string, { up: number; down: number }>>({});
  const [rates, setRates] = useState<Record<string, { up: number; down: number }>>({});
  const [names, setNames] = useState<Record<string, string | null>>({});
  const [elevated, setElevated] = useState<boolean | null>(null);
  const [adminHint, setAdminHint] = useState<string | null>(null);
  const focused = useWindowFocused();
  const lastPoll = useRef(0);
  const requested = useRef(new Set<string>());
  const prevConns = useRef(new Map<string, { up: number; down: number }>());

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
      const dtMs = lastPoll.current === 0 ? 0 : now - lastPoll.current;
      lastPoll.current = now;
      const dt = Math.min(dtMs, 30000);
      const seen = new Set<string>();
      r.forEach((e) => e.clients.forEach((c) => seen.add(c.ip)));
      setSessions((prev) => {
        const next: Record<string, Session> = { ...prev };
        seen.forEach((ip) => {
          const old = next[ip];
          next[ip] = old
            ? { firstSeen: old.firstSeen, totalMs: old.totalMs + (dt > 0 ? dt : 0) }
            : { firstSeen: now, totalMs: 0 };
        });
        return next;
      });
      if (dt > 0) {
        const dtS = dt / 1000;
        const nt: Record<string, { up: number; down: number }> = {};
        const nr: Record<string, { up: number; down: number }> = {};
        r.forEach((e) =>
          e.clients.forEach((c) => {
            if (c.bytes_up === null || c.bytes_down === null) return;
            const prev = prevConns.current.get(c.conn_id);
            const cur = { up: c.bytes_up, down: c.bytes_down };
            prevConns.current.set(c.conn_id, cur);
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
          setTotals((prev) => {
            const next = { ...prev };
            Object.entries(nt).forEach(([ip, d]) => {
              const o = next[ip] ?? { up: 0, down: 0 };
              next[ip] = { up: o.up + d.up, down: o.down + d.down };
            });
            return next;
          });
          setRates(nr);
        } else {
          setRates({});
        }
      }
    } catch {
      /* older backend: stay silent */
    } finally {
      setSpinning(false);
    }
  }, []);

  /* eslint-disable react-hooks/set-state-in-effect -- mount fetch + focus-gated poll */
  useEffect(() => {
    lastPoll.current = 0;
    void refresh();
    if (!focused) return;
    const id = setInterval(() => void refresh(), 10000);
    return () => clearInterval(id);
  }, [refresh, focused]);
  /* eslint-enable react-hooks/set-state-in-effect */

  const resolve = useCallback((ip: string) => {
    if (requested.current.has(ip)) return;
    requested.current.add(ip);
    invoke<string | null>("resolve_host", { ip })
      .then((n) => setNames((m) => ({ ...m, [ip]: n })))
      .catch(() => setNames((m) => ({ ...m, [ip]: null })));
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
