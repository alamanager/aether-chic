import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw, Users } from "lucide-react";
import { cn } from "@/lib/utils";

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

/**
 * Who is on your proxy right now: ESTABLISHED connections per endpoint,
 * grouped by remote IP (loopback = this machine, counted separately).
 * Polls while mounted; a failure just shows dashes, never an error.
 */
export function ClientsCard() {
  const [data, setData] = useState<EndpointClients[] | null>(null);
  const [spinning, setSpinning] = useState(false);

  const refresh = useCallback(async () => {
    setSpinning(true);
    try {
      const r = await invoke<EndpointClients[]>("proxy_clients");
      setData(r);
    } catch {
      /* backend without the command (older backend): stay silent */
    } finally {
      setSpinning(false);
    }
  }, []);

  /* eslint-disable react-hooks/set-state-in-effect -- initial fetch on
   * mount; polling afterwards. Same pattern as ConnectionInfo. */
  useEffect(() => {
    void refresh();
    const id = setInterval(() => void refresh(), 10000);
    return () => clearInterval(id);
  }, [refresh]);
  /* eslint-enable react-hooks/set-state-in-effect */

  const people = data?.reduce((n, e) => n + e.clients.length, 0) ?? null;

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
      ) : data.every((e) => e.total_connections === 0) ? (
        <p className="px-1 pb-1 text-[11px] text-muted-foreground">
          Nobody yet — share the SOCKS or HTTP address above.
        </p>
      ) : (
        data
          .filter((e) => e.total_connections > 0)
          .map((e) => (
            <div key={`${e.label}:${e.port}`} className="flex flex-col gap-0.5 px-1 pb-1">
              <div className="flex items-baseline justify-between text-[11px]">
                <span className="font-semibold text-foreground">
                  {e.label}{" "}
                  <span className="font-mono font-normal text-muted-foreground" dir="ltr">
                    :{e.port}
                  </span>
                </span>
                <span className="font-mono text-muted-foreground" dir="ltr">
                  {e.clients.length} {e.clients.length === 1 ? "person" : "people"} ·{" "}
                  {e.total_connections} conn
                  {e.local_connections > 0 ? ` (${e.local_connections} local)` : ""}
                </span>
              </div>
              {e.clients.slice(0, 6).map((c) => (
                <div
                  key={c.ip}
                  className="flex items-baseline justify-between font-mono text-[11px] text-muted-foreground"
                  dir="ltr"
                >
                  <span className="truncate">{c.ip}</span>
                  <span className="shrink-0">×{c.connections}</span>
                </div>
              ))}
            </div>
          ))
      )}
    </div>
  );
}
