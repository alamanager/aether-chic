import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useConnectionStore } from "@/state/connectionStore";
import type { ExtraTransport } from "@/types/connection";

const LABELS: Record<ExtraTransport, string> = {
  none: "Off",
  tor: "Tor inside the tunnel",
  tor_reverse: "Tunnel through Tor",
  tor_only: "Tor only",
  psiphon: "Psiphon inside the tunnel",
  psiphon_reverse: "Tunnel through Psiphon",
  psiphon_only: "Psiphon only",
  psiphon_direct: "Psiphon direct (console client)",
};

/**
 * One select for the mutually exclusive v2.x transports (core flags
 * --tor/--psiphon/...). Needs the core's pt/ transports beside the binary,
 * bundled since the v2.1.0 pin. Locked mid-session like the other profile
 * controls — changing transport requires a reconnect.
 */
export function ExtraTransportSelect() {
  const status = useConnectionStore((s) => s.status);
  const protocol = useConnectionStore((s) => s.profile.protocol);
  const extra = useConnectionStore((s) => s.profile.extra_transport);
  const setExtraTransport = useConnectionStore((s) => s.setExtraTransport);

  const locked = status.state !== "Idle" && status.state !== "Error";
  // gool-classic is WireGuard-based: reverse-mode transports are refused
  // with it, so the whole section locks (selecting it already cleared it).
  const disabled = locked || protocol === "gool_classic";

  return (
    <Select
      value={extra}
      onValueChange={(v) => setExtraTransport(v as ExtraTransport)}
      disabled={disabled}
    >
      <SelectTrigger
        size="sm"
        className="w-full border-transparent bg-transparent text-muted-foreground shadow-none hover:bg-surface-2"
        aria-label="Extra transport (Tor / Psiphon)"
      >
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {(Object.keys(LABELS) as ExtraTransport[]).map((t) => (
          <SelectItem key={t} value={t}>
            {LABELS[t]}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
