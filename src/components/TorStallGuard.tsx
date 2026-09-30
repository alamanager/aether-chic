import { useEffect, useRef, useState } from "react";
import { useConnectionStore } from "@/state/connectionStore";
import type { LogLine } from "@/types/connection";

const STALL_MS = 150_000;
const CHECK_MS = 5_000;

/** Last "tor reaching the network: N%" the engine reported, if any. */
function lastTorPercent(logs: LogLine[]): number | null {
  for (let i = logs.length - 1; i >= 0; i--) {
    const m = /reaching the network:\s*(\d+)%/.exec(logs[i].line);
    if (m) return Number(m[1]);
  }
  return null;
}

function isTorMode(extra: string): boolean {
  return extra === "tor" || extra === "tor_reverse" || extra === "tor_only";
}

/**
 * Ports the Android client's Tor lesson: the engine binds local ports long
 * before Tor reaches the network, so an open port proves nothing. While a
 * Tor attempt is running, watch the bootstrap percentage from the log
 * stream — any movement (even down, bridge retries restart low) counts as
 * alive. If it stalls past STALL_MS, disconnect with a message naming the
 * real cause instead of hanging until the 10-minute backstop.
 */
export function TorStallGuard() {
  const [msg, setMsg] = useState<string | null>(null);
  const tracked = useRef<{ pct: number | null; at: number } | null>(null);

  useEffect(() => {
    const id = setInterval(() => {
      const s = useConnectionStore.getState();
      const st = s.status.state;
      if (!isTorMode(s.profile.extra_transport)) return;
      if (st !== "Launching" && st !== "Connecting") return;
      const pct = lastTorPercent(s.logs);
      const now = Date.now();
      const prev = tracked.current;
      if (!prev || pct !== prev.pct) {
        tracked.current = { pct, at: now };
      } else if (pct !== null && now - prev.at > STALL_MS) {
        void s.disconnect();
        setMsg(
          `Tor stalled at ${pct}% for over 2 minutes — this network is likely blocking it. Turn on "Tor: straight to bridges" in Advanced and reconnect.`,
        );
        tracked.current = { pct, at: now };
      }
    }, CHECK_MS);
    return () => clearInterval(id);
  }, []);

  if (!msg) return null;
  return (
    <div className="glass w-full max-w-sm rounded-2xl px-4 py-2.5 text-center text-xs leading-5 text-muted-foreground">
      ⚠️ {msg}
    </div>
  );
}
