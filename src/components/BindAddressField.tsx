import { useConnectionStore } from "@/state/connectionStore";
import { Switch } from "@/components/ui/switch";

const DEFAULT_PORT = "1819";
const LOOPBACK = "127.0.0.1";
const ANY = "0.0.0.0";

function splitAddr(addr: string): { host: string; port: string } {
  const last = addr.lastIndexOf(":");
  if (last === -1) return { host: LOOPBACK, port: addr || DEFAULT_PORT };
  return { host: addr.slice(0, last) || LOOPBACK, port: addr.slice(last + 1) || DEFAULT_PORT };
}

function PortInput({
  value,
  onChange,
  label,
  disabled,
}: {
  value: string;
  onChange: (v: string) => void;
  label: string;
  disabled: boolean;
}) {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-[10px] font-medium tracking-wide text-muted-foreground uppercase">
        {label}
      </span>
      <input
        type="text"
        inputMode="numeric"
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value.replace(/\D/g, "").slice(0, 5))}
        onBlur={() => {
          const n = Number(value);
          if (!value || n < 1 || n > 65535) onChange("");
        }}
        placeholder="auto"
        dir="ltr"
        className="h-8 w-20 rounded-md bg-black/20 px-2 text-center font-mono text-xs text-foreground ring-1 ring-white/10 outline-none placeholder:text-muted-foreground/50 focus:ring-primary disabled:opacity-50 light:bg-black/5 light:ring-black/10"
        aria-label={label}
      />
    </label>
  );
}

export function BindAddressField() {
  const bind = useConnectionStore((s) => s.profile.bind_address);
  const setBindAddress = useConnectionStore((s) => s.setBindAddress);
  const httpPort = useConnectionStore((s) => s.profile.http_port);
  const setHttpPort = useConnectionStore((s) => s.setHttpPort);
  const status = useConnectionStore((s) => s.status);
  const locked = status.state !== "Idle" && status.state !== "Error";

  const { host, port } = splitAddr(bind);
  const lan = host === ANY;
  const clash = httpPort !== "" && httpPort === port;

  const rebuild = (h: string, p: string) =>
    setBindAddress(`${h}:${p || DEFAULT_PORT}`);

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-end gap-2">
        <PortInput value={port} onChange={(v) => rebuild(host, v)} label="SOCKS port" disabled={locked} />
        <PortInput value={httpPort} onChange={setHttpPort} label="HTTP port" disabled={locked} />
        <div className="flex flex-1 items-center justify-end gap-1.5 pb-1">
          <span className="text-xs text-muted-foreground">LAN share</span>
          <Switch
            checked={lan}
            onCheckedChange={(on) => rebuild(on ? ANY : LOOPBACK, port)}
            disabled={locked}
            aria-label="Allow connections from the LAN (both SOCKS and HTTP)"
          />
        </div>
      </div>
      <p className="min-h-4 text-[11px] leading-4 text-muted-foreground">
        {clash
          ? "HTTP port equals SOCKS port — HTTP falls back to SOCKS+1."
          : "Empty HTTP port = SOCKS+1. LAN share opens both endpoints to the network (no password!)."}
      </p>
    </div>
  );
}
