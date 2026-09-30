import type { ReactNode } from "react";
import { Info, Network, ShieldCheck, Zap } from "lucide-react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Switch } from "@/components/ui/switch";
import { ProtocolSelect } from "@/components/ProtocolSelect";
import { ScanModeToggle } from "@/components/ScanModeToggle";
import { IpVersionToggle } from "@/components/IpVersionToggle";
import { MasqueTransportToggle } from "@/components/MasqueTransportToggle";
import { NoizeProfileToggle } from "@/components/NoizeProfileToggle";
import { BindAddressField } from "@/components/BindAddressField";
import { UpstreamField } from "@/components/UpstreamField";
import { ExtraTransportSelect } from "@/components/ExtraTransportSelect";
import { PsiphonOptions } from "@/components/PsiphonOptions";
import { ZeroTrustSettings } from "@/components/ZeroTrustSettings";
import { RoutingSettings } from "@/components/RoutingSettings";
import { useConnectionStore } from "@/state/connectionStore";

function FieldRow({
  label,
  tooltip,
  children,
}: {
  label: string;
  tooltip?: string;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center gap-1 text-xs text-muted-foreground">
        {label}
        {tooltip && (
          <Tooltip>
            <TooltipTrigger aria-label={`About ${label}`}>
              <Info size={12} />
            </TooltipTrigger>
            <TooltipContent>{tooltip}</TooltipContent>
          </Tooltip>
        )}
      </div>
      {children}
    </div>
  );
}

function Group({
  icon,
  title,
  children,
}: {
  icon: ReactNode;
  title: string;
  children: ReactNode;
}) {
  return (
    <section className="glass w-full max-w-sm rounded-2xl p-4">
      <h2 className="mb-3 flex items-center gap-2 text-[13px] font-bold tracking-wide text-foreground">
        <span className="grid size-7 place-items-center rounded-lg bg-primary/15 text-primary">
          {icon}
        </span>
        {title}
      </h2>
      <div className="flex flex-col gap-4">{children}</div>
    </section>
  );
}

/**
 * Skips Tor's ~75s direct attempt and goes straight to bridges. Only
 * enabled with a Tor mode selected above (the backend gates the flag the
 * same way) — on a network that blocks Tor outright, direct is futile:
 * observed stuck at 15% fetching consensus.
 */
function TorBridgesRow() {
  const status = useConnectionStore((s) => s.status);
  const extra = useConnectionStore((s) => s.profile.extra_transport);
  const torBridges = useConnectionStore((s) => s.profile.tor_bridges);
  const setTorBridges = useConnectionStore((s) => s.setTorBridges);

  const locked = status.state !== "Idle" && status.state !== "Error";
  const isTor = extra === "tor" || extra === "tor_reverse" || extra === "tor_only";

  return (
    <div className="flex items-center justify-between pt-1">
      <span className="text-xs text-muted-foreground">Tor: straight to bridges</span>
      <Switch
        checked={torBridges}
        onCheckedChange={setTorBridges}
        disabled={locked || !isTor}
        aria-label="Skip Tor direct attempt, use bridges immediately"
      />
    </div>
  );
}

function QuickReconnectRow() {
  const status = useConnectionStore((s) => s.status);
  const quickReconnect = useConnectionStore((s) => s.profile.quick_reconnect);
  const setQuickReconnect = useConnectionStore((s) => s.setQuickReconnect);
  // Launch flag — locked mid-session like the other profile controls.
  const locked = status.state !== "Idle" && status.state !== "Error";

  return (
    <div className="flex items-center justify-between">
      <div className="flex items-center gap-1 text-xs text-muted-foreground">
        Quick reconnect
        <Tooltip>
          <TooltipTrigger aria-label="About Quick reconnect">
            <Info size={12} />
          </TooltipTrigger>
          <TooltipContent>
            Remembers the last gateway that worked and re-tests it first on the next connect,
            skipping the full scan when it still works. Turn off to always scan fresh.
          </TooltipContent>
        </Tooltip>
      </div>
      <Switch
        checked={quickReconnect}
        onCheckedChange={setQuickReconnect}
        disabled={locked}
        aria-label="Quick reconnect"
      />
    </div>
  );
}

/**
 * All profile controls, grouped by concern instead of one long Advanced
 * list. Every control keeps its Idle/Error lock: Aether can't reconfigure
 * mid-session, so changing anything requires a disconnect first.
 */
export function SettingsTab() {
  return (
    <div className="flex w-full flex-col items-center gap-3">
      <Group icon={<Zap className="size-4" />} title="Transport">
        <FieldRow
          label="Protocol"
          tooltip="MASQUE disguises traffic as normal HTTPS — best against strict censorship. WireGuard is lighter and faster. gool nests two WireGuard tunnels for extra security at a speed cost."
        >
          <ProtocolSelect />
        </FieldRow>
        <FieldRow label="Scan Mode">
          <ScanModeToggle />
        </FieldRow>
        <FieldRow
          label="Extra transport"
          tooltip="Built-in Tor or Psiphon from core v2.x — either carried inside the tunnel, used to reach the tunnel, or on its own. Needs the pt/ transports bundled with the app."
        >
          <ExtraTransportSelect />
          <TorBridgesRow />
          <PsiphonOptions />
        </FieldRow>
      </Group>

      <Group icon={<Network className="size-4" />} title="Network">
        <FieldRow
          label="IP Version"
          tooltip="Which address families to search for working routes. IPv4 is the safest default on most networks."
        >
          <IpVersionToggle />
        </FieldRow>
        <FieldRow
          label="MASQUE Transport"
          tooltip="How the MASQUE tunnel carries traffic. HTTP/3 (QUIC) has the fastest handshake; HTTP/2 (TCP) looks like ordinary HTTPS and works where UDP is blocked or throttled. Only applies to the MASQUE protocol."
        >
          <MasqueTransportToggle />
        </FieldRow>
        <FieldRow
          label="Obfuscation"
          tooltip="Disguises the handshake so DPI can't fingerprint the protocol. Heavier profiles send more decoy traffic — try escalating if the default doesn't connect. Options change based on the selected protocol."
        >
          <NoizeProfileToggle />
        </FieldRow>
        <FieldRow
          label="SOCKS5 Proxy"
          tooltip="The local address Aether's SOCKS5 proxy listens on. Change the port to avoid conflicts, or enable LAN to share the tunnel with other devices on your network."
        >
          <BindAddressField />
        </FieldRow>
        <FieldRow
          label="Upstream proxy (chain)"
          tooltip="Dial out through another proxy or VPN app already running on this machine, e.g. socks5://127.0.0.1:1080 or http://proxy:8080 with optional user:password@ credentials. Empty means direct."
        >
          <UpstreamField />
        </FieldRow>
        <FieldRow
          label="DNS & Routing"
          tooltip="Optional Aether 1.5 controls for DNS inside the tunnel and rules that block a destination or send it directly outside the tunnel."
        >
          <RoutingSettings />
        </FieldRow>
      </Group>

      <Group icon={<ShieldCheck className="size-4" />} title="Privacy">
        <FieldRow
          label="Zero Trust (organization)"
          tooltip="Connect as a managed Cloudflare Zero Trust device instead of anonymous consumer WARP. Works with MASQUE and WireGuard. Leave the team empty for normal one-click mode."
        >
          <ZeroTrustSettings />
        </FieldRow>
        <QuickReconnectRow />
      </Group>
    </div>
  );
}
