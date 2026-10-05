import { useState, type ReactNode } from "react";
import { ChevronDown, Fingerprint, Gauge, Globe2, Info, Network, Route, ShieldCheck, Zap } from "lucide-react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Switch } from "@/components/ui/switch";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
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
import { TextOpt, SwitchRow, SelectOpt } from "@/components/fields";
import { useConnectionStore as connectionStore } from "@/state/connectionStore";
import { cn } from "@/lib/utils";

function FieldRow({ label, tooltip, children }: { label: string; tooltip?: string; children: ReactNode }) {
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

function Section({ icon, title, stamp, defaultOpen, children }: { icon: ReactNode; title: string; stamp: string; defaultOpen?: boolean; children: ReactNode }) {
  const [open, setOpen] = useState(defaultOpen ?? false);
  return (
    <Collapsible open={open} onOpenChange={setOpen} className="glass w-full max-w-sm rounded-2xl">
      <CollapsibleTrigger className="flex w-full items-center gap-2.5 rounded-2xl px-4 py-3 text-left outline-none focus-visible:ring-2 focus-visible:ring-primary">
        <span className="grid size-8 shrink-0 place-items-center rounded-xl text-[#c4b5fd] ring-1 ring-white/10"
          style={{ background: "linear-gradient(135deg, rgb(34 211 238 / 0.16), rgb(168 85 247 / 0.16))" }}>
          {icon}
        </span>
        <span className="min-w-0 flex-1">
          <span className="block text-[13px] font-bold tracking-wide text-foreground">{title}</span>
          <span className="block truncate font-mono text-[10px] text-muted-foreground">{stamp}</span>
        </span>
        <ChevronDown className={cn("size-4 shrink-0 text-muted-foreground transition-transform", open && "rotate-180")} />
      </CollapsibleTrigger>
      <CollapsibleContent className="px-4 pb-4">
        <div className="flex flex-col gap-4 border-t border-white/5 pt-3 light:border-black/5">{children}</div>
      </CollapsibleContent>
    </Collapsible>
  );
}

function QuickReconnectRow() {
  const status = connectionStore((s) => s.status);
  const quickReconnect = connectionStore((s) => s.profile.quick_reconnect);
  const setQuickReconnect = connectionStore((s) => s.setQuickReconnect);
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
      <Switch checked={quickReconnect} onCheckedChange={setQuickReconnect} disabled={locked} aria-label="Quick reconnect" />
    </div>
  );
}

/** Tune tab: every profile control, folded into collapsible sections.
 * Same Idle/Error lock everywhere — Aether can't reconfigure mid-session. */
export function SettingsTab() {
  const p = connectionStore((s) => s.profile);
  const api = connectionStore.getState();
  const statusState = connectionStore((s) => s.status.state);
  const locked = statusState !== "Idle" && statusState !== "Error";

  return (
    <div className="flex w-full flex-col items-center gap-2.5">
      {locked && (
        <p className="w-full max-w-sm rounded-xl bg-status-connecting/10 px-3 py-2 text-center text-[11px] text-[#c4b5fd] ring-1 ring-[#a855f7]/25">
          Tunnel is live — disconnect to retune.
        </p>
      )}

      <Section icon={<Zap className="size-4" />} title="Transport" stamp="protocol · scan · overlay" defaultOpen>
        <FieldRow label="Protocol"
          tooltip="MASQUE disguises traffic as normal HTTPS. WireGuard is lighter and faster. gool nests a WireGuard identity inside MASQUE for a foreign exit; classic is WireGuard-in-WireGuard; mim is MASQUE-in-MASQUE.">
          <ProtocolSelect />
        </FieldRow>
        <FieldRow label="Scan Mode"><ScanModeToggle /></FieldRow>
        <FieldRow label="Extra transport"
          tooltip="Built-in Tor or Psiphon: carried inside the tunnel (exit on its own listener, 1819 keeps WARP), used to reach the tunnel, or on its own. Reverse modes force MASQUE. Needs pt/ bundled.">
          <ExtraTransportSelect />
          <PsiphonOptions />
        </FieldRow>
      </Section>

      <Section icon={<Route className="size-4" />} title="Manual endpoints" stamp="forced peers · skip scan">
        <FieldRow label="Forced peers"
          tooltip="Skip scanning: force a MASQUE/WireGuard peer, the gool inner peer, or classic-gool / mim hops (port required). Empty = scan.">
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
      </Section>

      <Section icon={<Network className="size-4" />} title="Network" stamp="ip · transport · share · route">
        <FieldRow label="IP Version"
          tooltip="Which address families to search for working routes. IPv4 is the safest default on most networks.">
          <IpVersionToggle />
        </FieldRow>
        <FieldRow label="MASQUE Transport"
          tooltip="How the MASQUE tunnel carries traffic. HTTP/3 (QUIC) has the fastest handshake; HTTP/2 (TCP) looks like ordinary HTTPS and works where UDP is blocked or throttled. Only applies to the MASQUE protocol.">
          <MasqueTransportToggle />
        </FieldRow>
        <FieldRow label="Obfuscation"
          tooltip="One list for every protocol since core v2.2. Heavier profiles send more decoy traffic — escalate when the default doesn't connect.">
          <NoizeProfileToggle />
        </FieldRow>
        <FieldRow label="Local endpoints (share)"
          tooltip="SOCKS5 port plus a separate HTTP port (empty = SOCKS+1, served natively by the core). LAN share opens BOTH endpoints to your network with no password — only for networks you trust.">
          <BindAddressField />
        </FieldRow>
        <FieldRow label="Upstream proxy (chain)"
          tooltip="Dial out through another proxy or VPN app already running on this machine, e.g. socks5://127.0.0.1:1080 or http://proxy:8080 with optional user:password@ credentials. Empty means direct.">
          <UpstreamField />
        </FieldRow>
        <FieldRow label="DNS & Routing"
          tooltip="Optional Aether 1.5 controls for DNS inside the tunnel and rules that block a destination or send it directly outside the tunnel.">
          <RoutingSettings />
        </FieldRow>
      </Section>

      <Section icon={<Gauge className="size-4" />} title="Performance" stamp="profile · mtu · checks">
        <FieldRow label="Resource profile" tooltip="Force low/medium/high instead of auto-detecting from CPU/RAM. Empty = auto.">
          <SelectOpt value={p.perf} onChange={api.setPerf} disabled={locked} label="Resource profile" options={[["", "Automatic"], ["low", "Low (routers)"], ["medium", "Desktop"], ["high", "Servers"]]} />
        </FieldRow>
        <FieldRow label="MTU & buffers" tooltip="Inner MASQUE MTU and netstack TCP buffers. Empty = core defaults (only touch when tuning).">
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
        <FieldRow label="H2 peer override" tooltip="Override the peer used for the HTTP/2 transport. Empty = automatic.">
          <TextOpt value={p.h2_peer} onChange={api.setH2Peer} disabled={locked} placeholder="ip:port (optional)" label="HTTP/2 peer override" mono />
        </FieldRow>
      </Section>

      <Section icon={<Globe2 className="size-4" />} title="Exit country" stamp="location filter">
        <FieldRow label="Exit filter"
          tooltip="Refuse tunnels whose exit country is unwanted — checked before SOCKS opens and every minute after. !IR,AZ,RU blocks those; DE,SE allows only those. Empty = off.">
          <TextOpt value={p.exit_loc} onChange={api.setExitLoc} disabled={locked} placeholder="!IR,AZ,RU or DE,SE (optional)" label="Exit country filter" mono />
        </FieldRow>
      </Section>

      <Section icon={<Fingerprint className="size-4" />} title="ECH & TLS" stamp="handshake disguise">
        <FieldRow label="Encrypted Client Hello"
          tooltip="Enable ECH on MASQUE handshakes and WARP API calls: auto looks the key up over DoH, or paste a base64 key. Empty = off.">
          <div className="flex flex-col gap-1.5">
            <TextOpt value={p.ech} onChange={api.setEch} disabled={locked} placeholder="auto or base64 key (optional)" label="ECH mode" mono />
            <TextOpt value={p.ech_dns} onChange={api.setEchDns} disabled={locked} placeholder="Key resolver (default udp://1.1.1.1)" label="ECH DNS resolver" mono />
            <TextOpt value={p.ech_domain} onChange={api.setEchDomain} disabled={locked} placeholder="Key domain (default cloudflare-ech.com)" label="ECH domain" mono />
          </div>
        </FieldRow>
        <FieldRow label="TLS fingerprint" tooltip="TLS 1.2 cipher suites and groups (Chrome defaults when empty). GREASE values are included like Chrome unless disabled.">
          <div className="flex flex-col gap-1.5">
            <TextOpt value={p.tls_ciphers} onChange={api.setTlsCiphers} disabled={locked} placeholder="Cipher list (optional)" label="TLS ciphers" mono />
            <TextOpt value={p.tls_groups} onChange={api.setTlsGroups} disabled={locked} placeholder="Groups, e.g. P-256:X25519:P-384" label="TLS groups" mono />
          </div>
        </FieldRow>
        <SwitchRow checked={!p.disable_grease} onChange={(v) => api.setDisableGrease(!v)} disabled={locked} label="GREASE values (Chrome-like)" />
        <SwitchRow checked={p.route_sniff} onChange={api.setRouteSniff} disabled={locked} label="SNI sniffing for routing rules" />
      </Section>

      <Section icon={<ShieldCheck className="size-4" />} title="Privacy" stamp="zero trust · reconnect">
        <FieldRow label="Zero Trust (organization)"
          tooltip="Connect as a managed Cloudflare Zero Trust device instead of anonymous consumer WARP. Works with MASQUE and WireGuard. Leave the team empty for normal one-click mode.">
          <ZeroTrustSettings />
        </FieldRow>
        <QuickReconnectRow />
      </Section>
    </div>
  );
}
