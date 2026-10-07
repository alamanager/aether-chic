import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { FlaskConical, Play, Square } from "lucide-react";
import { cn } from "@/lib/utils";
import { useConnectionStore, setMatrixActive } from "@/state/connectionStore";

interface MatrixEvent {
  key: string;
  protocol: string;
  transport: string;
  phase: "running" | "done" | "finished";
  ok: boolean;
  ms: number | null;
  exit: string | null;
  note: string;
}

const PROTOCOLS = ["auto", "masque", "wireguard", "gool", "gool-classic", "mim"] as const;
const TRANSPORTS = ["warp", "psiphon", "psiphon-only"] as const;

const toStoreProtocol = (p: string): string => (p === "gool-classic" ? "gool_classic" : p);
const toStoreTransport = (t: string): string => (t === "warp" ? "none" : t.replace("-", "_"));

function fmtMs(ms: number | null): string {
  if (ms === null) return "—";
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

/**
 * Connection matrix lab: connects each of the 30 protocol × transport
 * combos for real (fresh scan each, per-type budgets), measures
 * through-tunnel latency on success. Slow by nature (Tor bootstraps!) —
 * cancel anytime. Reverses excluded: MASQUE-only, covered by masque rows.
 */
export function MatrixTab() {
  const [results, setResults] = useState<Record<string, MatrixEvent>>({});
  const [running, setRunning] = useState(false);
  const [hint, setHint] = useState<string | null>(null);
  const setProtocol = useConnectionStore((s) => s.setProtocol);
  const setExtraTransport = useConnectionStore((s) => s.setExtraTransport);

  useEffect(() => {
    const unlisten = listen<MatrixEvent>("aether://matrix", (e) => {
      const ev = e.payload;
      if (ev.phase === "finished") {
        setRunning(false);
        setMatrixActive(false);
        return;
      }
      if (ev.phase === "running") setRunning(true);
      setResults((prev) => ({ ...prev, [ev.key]: ev }));
    });
    return () => {
      // Leaving Lab aborts the run and unfreezes the main UI.
      void invoke("matrix_cancel").catch(() => {});
      setMatrixActive(false);
      void unlisten.then((u) => u());
    };
  }, []);

  const start = async () => {
    setHint(null);
    setResults({});
    try {
      setMatrixActive(true);
      await invoke("matrix_start");
      setRunning(true);
    } catch (e) {
      setMatrixActive(false);
      setHint(`Could not start: ${String(e)}. Disconnect first, then test.`);
    }
  };

  const cancel = async () => {
    setMatrixActive(false);
    try {
      await invoke("matrix_cancel");
    } catch {
      /* already stopped */
    }
  };

  const use = (protocol: string, transport: string) => {
    setProtocol(toStoreProtocol(protocol) as never);
    setExtraTransport(toStoreTransport(transport) as never);
    setHint(`Profile set to ${protocol} + ${transport} — hit connect on Pulse.`);
  };

  const done = Object.values(results).filter((r) => r.phase === "done");
  const okCount = done.filter((r) => r.ok).length;

  return (
    <div className="flex w-full flex-col items-center gap-3">
      <div className="glass flex w-full max-w-sm flex-col gap-2 rounded-2xl p-3">
        <div className="flex h-6 items-center gap-2 px-1">
          <FlaskConical className="size-4 text-[#a5b4fc]" />
          <span className="text-[10px] font-bold tracking-[0.2em] text-muted-foreground uppercase">
            Connection matrix
          </span>
          {done.length > 0 && (
            <span className="rounded-full bg-status-connected/15 px-2 py-0.5 font-mono text-[10px] font-bold text-status-connected">
              {okCount}/{done.length}
            </span>
          )}
        </div>
        <p className="px-1 text-[11px] leading-5 text-muted-foreground">
          Tries all 18 combos at once (no Tor rows — Tor never answers
          fast) and times a fetch through each tunnel. Cancel anytime —
          finished rows stay. Leaving this tab aborts the run.
        </p>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => void start()}
            disabled={running}
            className="flex flex-1 items-center justify-center gap-1.5 rounded-xl bg-surface-3 px-2 py-2 text-xs font-bold text-foreground transition-colors hover:bg-surface-4 disabled:opacity-50"
          >
            <Play className="size-3.5" />
            {running ? "Testing…" : "Test all"}
          </button>
          <button
            type="button"
            onClick={() => void cancel()}
            disabled={!running}
            aria-label="Cancel matrix"
            className="grid size-9 shrink-0 place-items-center rounded-xl text-muted-foreground ring-1 ring-white/10 transition-colors hover:bg-surface-3 hover:text-foreground disabled:opacity-50"
          >
            <Square className="size-3.5" />
          </button>
        </div>
        {hint && <p className="px-1 text-[11px] text-muted-foreground">{hint}</p>}
      </div>
      <div className="glass flex w-full max-w-sm flex-col gap-0.5 rounded-2xl p-3">
        {PROTOCOLS.map((p) => (
          <div key={p} className="flex flex-col">
            <p className="px-1 pt-1 font-mono text-[10px] font-bold tracking-widest text-muted-foreground uppercase" dir="ltr">
              {p}
            </p>
            {TRANSPORTS.map((t) => {
              const r = results[`${p}+${t}`];
              return (
                <div key={t} className="flex items-center gap-2 rounded-lg px-1 py-1">
                  <span
                    className={cn(
                      "size-2 shrink-0 rounded-full",
                      !r || r.phase === "running"
                        ? r
                          ? "anim-glow-fast bg-status-connecting"
                          : "bg-status-idle"
                        : r.ok
                          ? "bg-status-connected"
                          : "bg-status-error",
                    )}
                    aria-hidden
                  />
                  <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-foreground" dir="ltr">
                    {t}
                  </span>
                  <span className="shrink-0 font-mono text-[10px] text-muted-foreground" dir="ltr">
                    {r && r.phase === "done" ? (r.ok ? fmtMs(r.ms) : (r.note || "fail").slice(0, 24)) : r ? "…" : ""}
                  </span>
                  {r?.ok && (
                    <button
                      type="button"
                      onClick={() => use(p, t)}
                      className="shrink-0 rounded-lg bg-surface-3 px-2 py-0.5 text-[10px] font-bold text-foreground transition-colors hover:bg-surface-4"
                    >
                      Use
                    </button>
                  )}
                </div>
              );
            })}
          </div>
        ))}
      </div>
    </div>
  );
}
