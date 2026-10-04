import type { ReactNode } from "react";
import { Fingerprint, Gauge, Globe2, Info, Network, Route, ShieldCheck, Zap } from "lucide-react";
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
import { TextOpt, AreaOpt, SwitchRow, SelectOpt } from "@/components/fields";
import { useConnectionStore as connectionStore } from "@/state/connectionStore";

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
  delay,
  children,
}: {
  icon: ReactNode;
  title: string;
  delay?: number;
  children: ReactNode;
}) {
  return (
    <section
      className="glass anim-enter w-full max-w-sm rounded-2xl p-4"
      style={delay ? { animationDelay: `${delay}ms` } : undefined}
    >
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
  const status = connectionStore((s) => s.status);
  const extra = connectionStore((s) => s.profile.extra_transport);
  const torBridges = connectionStore((s) => s.profile.tor_bridges);
  const setTorBridges = connectionStore((s) => s.setTorBridges);

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
  const status = connectionStore((s) => s.status);
  const quickReconnect = connectionStore((s) => s.profile.quick_reconnect);
  const setQuickReconnect = connectionStore((s) => s.setQuickReconnect);
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
  // Values subscribed (re-render on change); setters are stable references
  // read once via getState so log-stream updates never re-render this tab.
  const p = connectionStore((s) => s.profile);
  const api = connectionStore.getState();
  const locked =
    connectionStore((s) => s.status).state !== "Idle" &&
    connectionStore((s) => s.status).state !== "Error";

  return (
    <div className="flex w-full flex-col items-center gap-3">
      <Group icon={<Zap className="size-4" />} title="Transport" delay={0}>
        <FieldRow
          label="Protocol"
          tooltip="MASQUE disguises traffic as normal HTTPS. WireGuard is lighter and faster. gool nests a WireGuard identity inside MASQUE for a foreign exit; classic is WireGuard-in-WireGuard; mim is MASQUE-in-MASQUE."
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

      <Group icon={<Network className="size-4" />} title="Tor options" delay={60}>
        <FieldRow
          label="Relays mode"
          tooltip="auto = bridgedb + onionoo relays alongside plain Tor; only = relays at once (skips the plain attempt); off = never. A count (e.g. 80) limits the pool."
        >
          <TextOpt value={p.tor_relays} onChange={api.setTorRelays} disabled={locked} placeholder="auto" label="Tor relays mode" mono />
        </FieldRow>
        <FieldRow
          label="Custom bridges"
          tooltip="Your own bridge lines, one per line — same shape torrc uses. Tried instead of (or before) fetched ones."
        >
          <AreaOpt value={p.tor_bridge} onChange={api.setTorBridge} disabled={locked} placeholder={"obfs4 1.2.3.4:443 FINGERPRINT cert=... iat-mode=0"} label="Custom Tor bridges" />
        </FieldRow>
        <FieldRow
          label="Bridge file / Tor listener"
          tooltip="Bridge file: path to a torrc-shaped file. Tor listener: where the Tor exit of --tor/--tor-reverse serves (default 127.0.0.1:1820 — keep clear of the HTTP port!)."
        >
          <div className="flex flex-col gap-1.5">
            <TextOpt value={p.tor_bridge_file} onChange={api.setTorBridgeFile} disabled={locked} placeholder="Bridge file path (optional)" label="Tor bridge file" mono />
            <TextOpt value={p.tor_bind} onChange={api.setTorBind} disabled={locked} placeholder="127.0.0.1:1820" label="Tor exit listener" mono />
          </div>
        </FieldRow>
      </Group>

      <Group icon={<Network className="size-4" />} title="Psiphon files" delay={90}>
        <FieldRow
          label="Custom config / server entries"
          tooltip="Your own psiphon JSON laid over the built-in one (credentials/server lists), or a seed server-entries file so the first connection skips the list download."
        >
          <div className="flex flex-col gap-1.5">
            <TextOpt value={p.psiphon_config} onChange={api.setPsiphonConfig} disabled={locked} placeholder="Custom config path (optional)" label="Psiphon custom config" mono />
            <TextOpt value={p.psiphon_server_entries} onChange={api.setPsiphonServerEntries} disabled={locked} placeholder="Server entries file (optional)" label="Psiphon server entries" mono />
          </div>
        </FieldRow>
        <FieldRow
          label="CDN fronting"
          tooltip="Override which CDN edges / names / sets the fronting scan tries. Empty = built-in lists."
        >
          <div className="flex flex-col gap-1.5">
            <TextOpt value={p.psiphon_cdn_ips} onChange={api.setPsiphonCdnIps} disabled={locked} placeholder="CDN edge IPs (optional)" label="Psiphon CDN IPs" mono />
            <TextOpt value={p.psiphon_cdn_sni} onChange={api.setPsiphonCdnSni} disabled={locked} placeholder="Server names (optional)" label="Psiphon CDN SNI" mono />
            <TextOpt value={p.psiphon_cdn_sets} onChange={api.setPsiphonCdnSets} disabled={locked} placeholder="Edge sets, e.g. cloudflare,fastly (optional)" label="Psiphon CDN sets" />
          </div>
        </FieldRow>
        <FieldRow
          label="Psiphon listener"
          tooltip="Where the Psiphon exit of --psiphon/--psiphon-reverse serves (default 127.0.0.1:1821)."
        >
          <TextOpt value={p.psiphon_bind} onChange={api.setPsiphonBind} disabled={locked} placeholder="127.0.0.1:1821" label="Psiphon exit listener" mono />
        </FieldRow>
      </Group>

      <Group icon={<Route className="size-4" />} title="Manual endpoints" delay={120}>
        <FieldRow
          label="Forced peers"
          tooltip="Skip scanning: force a MASQUE/WireGuard peer, the gool inner peer, or classic-gool / mim hops (port required). Empty = scan."
        >
          <div className="flex flex-col gap-1.5">
            <TextOpt value={p.peer} onChange={api.setPeer} disabled={locked} placeholder="peer ip:port (optional)" label="Forced peer" mono />
            <TextOpt value={p.wg_peer} onChange={api.setWgPeer} disabled={locked} placeholder="WireGuard peer ip:port" label="WireGuard peer" mono />
            <TextOpt value={p.gool_peer} onChange={api.setGoolPeer} disabled={locked} placeholder="gool inner peer ip:port" label="gool inner peer" mono />
            <TextOpt value={p.wiw_outer} onChange={api.setWiwOuter} disabled={locked} placeholder="classic outer hop ip:port" label="Classic outer hop" mono />
            <TextOpt value={p.wiw_inner} onChange={api.setWiwInner} disabled={locked} placeholder="classic inner hop ip:port" label="Classic inner hop" mono />
            <TextOpt value={p.mim_outer} onChange={api.setMimOuter} disabled={locked} placeholder="mim outer hop ip:port" label="mim outer hop" mono />
            <TextOpt value={p.mim_inner} onChange={api.setMimInner} disabled={locked} placeholder="mim inner hop ip:port" label="mim inner hop" mono />
          </div>
        </FieldRow>
      </Group>

      <Group icon={<Network className="size-4" />} title="Network" delay={150}>
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
          tooltip="One list for every protocol since core v2.2. Heavier profiles send more decoy traffic — escalate when the default doesn't connect."
        >
          <NoizeProfileToggle />
        </FieldRow>
        <FieldRow
          label="Local endpoints (share)"
          tooltip="SOCKS5 port plus a separate HTTP port (empty = SOCKS+1, served natively by the core). LAN share opens BOTH endpoints to your network with no password — only for networks you trust."
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

      <Group icon={<Gauge className="size-4" />} title="Performance" delay={180}>
        <FieldRow
          label="Resource profile"
          tooltip="Force low/medium/high instead of auto-detecting from CPU/RAM. Empty = auto."
        >
          <SelectOpt value={p.perf} onChange={api.setPerf} disabled={locked} label="Resource profile" options={[["", "Automatic"], ["low", "Low (routers)"], ["medium", "Desktop"], ["high", "Servers"]]} />
        </FieldRow>
        <FieldRow
          label="MTU & buffers"
          tooltip="Inner MASQUE MTU and netstack TCP buffers. Empty = core defaults (only touch when tuning)."
        >
          <div className="flex flex-col gap-1.5">
            <TextOpt value={p.masque_mtu} onChange={api.setMasqueMtu} disabled={locked} placeholder="MASQUE MTU (optional)" label="MASQUE MTU" mono />
            <TextOpt value={p.netstack_rx} onChange={api.setNetstackRx} disabled={locked} placeholder="TCP RX buffer (optional)" label="Netstack TCP RX" mono />
            <TextOpt value={p.netstack_tx} onChange={api.setNetstackTx} disabled={locked} placeholder="TCP TX buffer (optional)" label="Netstack TCP TX" mono />
            <TextOpt value={p.keepalive} onChange={api.setKeepalive} disabled={locked} placeholder="WG keepalive secs (default 5)" label="WireGuard keepalive" mono />
          </div>
        </FieldRow>
        <SwitchRow checked={!p.no_profile_retry} onChange={(v) => api.setNoProfileRetry(!v)} disabled={locked} label="Retry other obfuscation profiles" />
        <SwitchRow checked={!p.no_data_check} onChange={(v) => api.setNoDataCheck(!v)} disabled={locked} label="End-to-end data validation" />
        <SwitchRow checked={!p.no_quic_v2} onChange={(v) => api.setNoQuicV2(!v)} disabled={locked} label="QUIC v2 opener" />
        <SwitchRow checked={p.api_fragment} onChange={api.setApiFragment} disabled={locked} label="Fragment WARP API route" />
        <FieldRow
          label="H2 peer override"
          tooltip="Override the peer used for the HTTP/2 transport. Empty = automatic."
        >
          <TextOpt value={p.h2_peer} onChange={api.setH2Peer} disabled={locked} placeholder="ip:port (optional)" label="HTTP/2 peer override" mono />
        </FieldRow>
      </Group>

      <Group icon={<Globe2 className="size-4" />} title="Exit country" delay={210}>
        <FieldRow
          label="Exit filter"
          tooltip="Refuse tunnels whose exit country is unwanted — checked before SOCKS opens and every minute after. !IR,AZ,RU blocks those; DE,SE allows only those. Empty = off."
        >
          <TextOpt value={p.exit_loc} onChange={api.setExitLoc} disabled={locked} placeholder="!IR,AZ,RU or DE,SE (optional)" label="Exit country filter" mono />
        </FieldRow>
      </Group>

      <Group icon={<Fingerprint className="size-4" />} title="ECH & TLS" delay={240}>
        <FieldRow
          label="Encrypted Client Hello"
          tooltip="Enable ECH on MASQUE handshakes and WARP API calls: auto looks the key up over DoH, or paste a base64 key. Empty = off."
        >
          <div className="flex flex-col gap-1.5">
            <TextOpt value={p.ech} onChange={api.setEch} disabled={locked} placeholder="auto or base64 key (optional)" label="ECH mode" mono />
            <TextOpt value={p.ech_dns} onChange={api.setEchDns} disabled={locked} placeholder="Key resolver (default udp://1.1.1.1)" label="ECH DNS resolver" mono />
            <TextOpt value={p.ech_domain} onChange={api.setEchDomain} disabled={locked} placeholder="Key domain (default cloudflare-ech.com)" label="ECH domain" mono />
          </div>
        </FieldRow>
        <FieldRow
          label="TLS fingerprint"
          tooltip="TLS 1.2 cipher suites and groups (Chrome defaults when empty). GREASE values are included like Chrome unless disabled."
        >
          <div className="flex flex-col gap-1.5">
            <TextOpt value={p.tls_ciphers} onChange={api.setTlsCiphers} disabled={locked} placeholder="Cipher list (optional)" label="TLS ciphers" mono />
            <TextOpt value={p.tls_groups} onChange={api.setTlsGroups} disabled={locked} placeholder="Groups, e.g. P-256:X25519:P-384" label="TLS groups" mono />
          </div>
        </FieldRow>
        <SwitchRow checked={!p.disable_grease} onChange={(v) => api.setDisableGrease(!v)} disabled={locked} label="GREASE values (Chrome-like)" />
        <SwitchRow checked={p.route_sniff} onChange={api.setRouteSniff} disabled={locked} label="SNI sniffing for routing rules" />
      </Group>

      <Group icon={<ShieldCheck className="size-4" />} title="Privacy" delay={270}>
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
