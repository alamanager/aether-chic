import { useMemo } from "react";
import {
  Area,
  CartesianGrid,
  ComposedChart,
  ReferenceDot,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { useConnectionStore } from "@/state/connectionStore";

function fmtAxis(bytes: number): string {
  if (bytes < 1024) return `${Math.round(bytes)}`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)}K`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)}M`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)}G`;
}

function fmtFull(bytes: number): string {
  if (bytes < 1024) return `${Math.round(bytes)} B/s`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB/s`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB/s`;
}

function fmtClock(t: number): string {
  return new Date(t).toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

/**
 * Fixed-height transfer history (~4 min ring buffer): download as a solid
 * teal area, upload as a dashed orange line (never hue alone, per the
 * design system). Disconnects render as visible gaps (null samples break
 * both series); the last gap gets a red marker dot. Time runs left→right.
 */
export function TransferGraph() {
  const samples = useConnectionStore((s) => s.samples);
  const reduced =
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  const gapAt = useMemo(() => {
    for (let i = samples.length - 1; i >= 0; i--) {
      if (samples[i].down === null) return samples[i].t;
    }
    return null;
  }, [samples]);

  const tipStyle = {
    background: "var(--color-card)",
    border: "1px solid var(--color-border)",
    borderRadius: 10,
    color: "var(--color-foreground)",
    fontSize: 11,
    fontFamily: "var(--font-mono)",
  } as const;

  return (
    <div className="glass flex h-[210px] w-full max-w-sm flex-col rounded-2xl p-3">
      <div className="mb-1 flex h-6 items-center justify-between px-1">
        <span className="text-[10px] font-semibold tracking-widest text-muted-foreground uppercase">
          Speed history
        </span>
        <span className="flex items-center gap-3 font-mono text-[10px] text-muted-foreground">
          <span className="flex items-center gap-1">
            <span className="inline-block h-0.5 w-3 rounded bg-status-connected" aria-hidden /> down
          </span>
          <span className="flex items-center gap-1">
            <span
              className="inline-block h-0 w-3 border-t-2 border-dashed border-status-connecting"
              aria-hidden
            />{" "}
            up
          </span>
        </span>
      </div>
      <div className="min-h-0 flex-1" role="img" aria-label="Download and upload speed over the last minutes">
        {samples.length < 2 ? (
          <div className="grid h-full place-items-center text-xs text-muted-foreground">
            No data yet — connect to record speed.
          </div>
        ) : (
          <ResponsiveContainer width="100%" height="100%">
            <ComposedChart data={samples} margin={{ top: 4, right: 4, bottom: 0, left: 0 }}>
              <CartesianGrid stroke="var(--color-border)" strokeDasharray="3 6" vertical={false} />
              <XAxis
                dataKey="t"
                tickFormatter={(t: number) => {
                  const last = samples[samples.length - 1]?.t ?? t;
                  const s = Math.max(0, Math.round((last - t) / 1000));
                  return s < 60 ? `-${s}s` : `-${Math.floor(s / 60)}m`;
                }}
                tick={{ fontSize: 9, fill: "var(--color-muted-foreground)" }}
                tickLine={false}
                axisLine={false}
                minTickGap={48}
                interval="preserveStartEnd"
              />
              <YAxis
                width={36}
                tickFormatter={fmtAxis}
                tick={{ fontSize: 9, fill: "var(--color-muted-foreground)" }}
                tickLine={false}
                axisLine={false}
              />
              <Tooltip
                contentStyle={tipStyle}
                labelFormatter={(t) => fmtClock(Number(t))}
                formatter={(v, name) => [
                  typeof v === "number" ? fmtFull(v) : "—",
                  name === "down" ? "down" : "up",
                ]}
              />
              <Area
                type="monotone"
                dataKey="down"
                name="down"
                stroke="var(--color-status-connected)"
                strokeWidth={2}
                fill="var(--color-status-connected)"
                fillOpacity={0.2}
                connectNulls={false}
                dot={false}
                activeDot={{ r: 3 }}
                isAnimationActive={!reduced}
              />
              <Area
                type="monotone"
                dataKey="up"
                name="up"
                stroke="var(--color-status-connecting)"
                strokeWidth={2}
                strokeDasharray="6 3"
                fill="none"
                connectNulls={false}
                dot={false}
                activeDot={{ r: 3 }}
                isAnimationActive={!reduced}
              />
              {gapAt !== null && (
                <ReferenceDot
                  x={gapAt}
                  y={0}
                  r={4}
                  fill="var(--color-status-error)"
                  stroke="none"
                />
              )}
            </ComposedChart>
          </ResponsiveContainer>
        )}
      </div>
    </div>
  );
}
