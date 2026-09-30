import { useConnectionStore } from "@/state/connectionStore";

export function UpstreamField() {
  const upstream = useConnectionStore((s) => s.profile.upstream);
  const setUpstream = useConnectionStore((s) => s.setUpstream);
  const status = useConnectionStore((s) => s.status);
  const locked = status.state !== "Idle" && status.state !== "Error";

  return (
    <input
      type="text"
      value={upstream}
      disabled={locked}
      spellCheck={false}
      onChange={(e) => setUpstream(e.target.value)}
      placeholder="socks5://127.0.0.1:1080"
      dir="ltr"
      className="h-8 w-full rounded-md bg-black/20 px-2 text-left font-mono text-xs text-foreground ring-1 ring-white/10 outline-none placeholder:text-muted-foreground/60 focus:ring-primary disabled:opacity-50 light:bg-black/5 light:ring-black/10"
      aria-label="Upstream proxy URL"
    />
  );
}
