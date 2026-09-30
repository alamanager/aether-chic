import { useEffect, useRef, useState } from "react";
import { Trash2 } from "lucide-react";
import { useConnectionStore } from "@/state/connectionStore";

/**
 * Dedicated log view: the raw core output stream with autoscroll that
 * yields to manual scrolling, plus a clear button. Previously buried at the
 * bottom of the Advanced panel.
 */
export function LogsTab() {
  const logs = useConnectionStore((s) => s.logs);
  const clearLogs = useConnectionStore((s) => s.clearLogs);
  const [autoScroll, setAutoScroll] = useState(true);
  const viewportRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (autoScroll && viewportRef.current) {
      viewportRef.current.scrollTop = viewportRef.current.scrollHeight;
    }
  }, [logs, autoScroll]);

  return (
    <div className="glass flex max-h-full w-full max-w-sm flex-col rounded-2xl p-3">
      <div className="mb-2 flex items-center justify-between px-1">
        <span className="text-[10px] font-semibold tracking-widest text-muted-foreground uppercase">
          Core output · {logs.length} lines
        </span>
        <button
          type="button"
          onClick={clearLogs}
          aria-label="Clear logs"
          className="grid size-8 place-items-center rounded-lg text-muted-foreground transition-colors hover:bg-surface-3 hover:text-foreground"
        >
          <Trash2 className="size-3.5" />
        </button>
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
          logs.map((l, i) => <p key={i}>{l.line}</p>)
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
