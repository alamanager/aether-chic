import { memo, useEffect, useRef, useState } from "react";
import { Copy, Check, Trash2 } from "lucide-react";
import { useConnectionStore } from "@/state/connectionStore";

/** Render window: full history stays in the store for copy, but only the
 * tail hits the DOM — 500 <p> nodes re-created per 100ms batch was the
 * main scroll jank. */
const RENDER_TAIL = 150;

function CopyButton({ logs }: { logs: { line: string }[] }) {
  const [done, setDone] = useState(false);
  return (
    <button
      type="button"
      onClick={() => {
        void navigator.clipboard
          ?.writeText(logs.map((l) => l.line).join("\n"))
          .then(() => {
            setDone(true);
            setTimeout(() => setDone(false), 1500);
          })
          .catch(() => {});
      }}
      aria-label="Copy logs"
      className="grid size-8 place-items-center rounded-lg text-muted-foreground transition-colors hover:bg-surface-3 hover:text-foreground"
    >
      {done ? <Check className="size-3.5 text-status-connected" /> : <Copy className="size-3.5" />}
    </button>
  );
}

const LogRow = memo(function LogRow({ line }: { line: string }) {
  return <p>{line}</p>;
});

/**
 * Dedicated log view: windowed tail render with stable keys, rAF-batched
 * autoscroll (scrollTop writes no longer fight React's commit phase),
 * plus copy and clear actions.
 */
export function LogsTab() {
  const logs = useConnectionStore((s) => s.logs);
  const clearLogs = useConnectionStore((s) => s.clearLogs);
  const [autoScroll, setAutoScroll] = useState(true);
  const viewportRef = useRef<HTMLDivElement>(null);
  const rafRef = useRef(0);

  useEffect(() => {
    if (!autoScroll) return;
    cancelAnimationFrame(rafRef.current);
    rafRef.current = requestAnimationFrame(() => {
      const el = viewportRef.current;
      if (el) el.scrollTop = el.scrollHeight;
    });
    return () => cancelAnimationFrame(rafRef.current);
  }, [logs, autoScroll]);

  const tail = logs.length > RENDER_TAIL ? logs.slice(-RENDER_TAIL) : logs;
  const skipped = logs.length - tail.length;

  return (
    <div className="glass flex max-h-full w-full max-w-sm flex-col rounded-2xl p-3">
      <div className="mb-2 flex items-center justify-between px-1">
        <span className="text-[10px] font-semibold tracking-widest text-muted-foreground uppercase">
          Core output · {logs.length} lines
        </span>
        <span className="flex items-center gap-1">
          <CopyButton logs={logs} />
          <button
            type="button"
            onClick={clearLogs}
            aria-label="Clear logs"
            className="grid size-8 place-items-center rounded-lg text-muted-foreground transition-colors hover:bg-surface-3 hover:text-foreground"
          >
            <Trash2 className="size-3.5" />
          </button>
        </span>
      </div>
      <div
        ref={viewportRef}
        onScroll={(e) => {
          const el = e.currentTarget;
          setAutoScroll(el.scrollHeight - el.scrollTop - el.clientHeight < 24);
        }}
        className="min-h-48 flex-1 overflow-y-auto rounded-xl bg-black/25 p-2.5 font-mono text-xs text-muted-foreground ring-1 ring-white/10 light:bg-black/5 light:ring-black/10"
      >
        {logs.length === 0 ? (
          <p className="text-status-idle">No output yet — connect to see the core log.</p>
        ) : (
          <>
            {skipped > 0 && (
              <p className="text-status-idle">… {skipped} older lines hidden (copy keeps all)</p>
            )}
            {tail.map((l, i) => (
              <LogRow key={`${l.timestamp}-${i}`} line={l.line} />
            ))}
          </>
        )}
      </div>
      {!autoScroll && (
        <button
          type="button"
          onClick={() => {
            setAutoScroll(true);
            if (viewportRef.current) {
              viewportRef.current.scrollTop = viewportRef.current.scrollHeight;
            }
          }}
          className="mt-2 rounded-xl bg-surface-3 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-surface-4"
        >
          ↓ Jump to latest
        </button>
      )}
    </div>
  );
}
