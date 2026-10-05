import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useConnectionStore } from "@/state/connectionStore";
import type { ConnectionProfile } from "@/types/connection";

// Psiphon's published egress regions. "" = automatic. The core treats a
// choice as a hard filter, so a region with no current exit won't
// connect — retry with Automatic.
const REGIONS: [string, string][] = [
  ["", "Automatic"],
  ["AE", "UAE"], ["AR", "Argentina"], ["AT", "Austria"], ["AU", "Australia"],
  ["BE", "Belgium"], ["BG", "Bulgaria"], ["BR", "Brazil"], ["CA", "Canada"],
  ["CH", "Switzerland"], ["CL", "Chile"], ["CO", "Colombia"], ["CY", "Cyprus"],
  ["CZ", "Czechia"], ["DE", "Germany"], ["DK", "Denmark"], ["EE", "Estonia"],
  ["ES", "Spain"], ["FI", "Finland"], ["FR", "France"], ["GB", "United Kingdom"],
  ["GR", "Greece"], ["HK", "Hong Kong"], ["HR", "Croatia"], ["HU", "Hungary"],
  ["IE", "Ireland"], ["IL", "Israel"], ["IN", "India"], ["IS", "Iceland"],
  ["IT", "Italy"], ["JP", "Japan"], ["KR", "South Korea"], ["LT", "Lithuania"],
  ["LU", "Luxembourg"], ["LV", "Latvia"], ["MD", "Moldova"], ["MX", "Mexico"],
  ["MY", "Malaysia"], ["NL", "Netherlands"], ["NO", "Norway"], ["NZ", "New Zealand"],
  ["PH", "Philippines"], ["PL", "Poland"], ["PT", "Portugal"], ["RO", "Romania"],
  ["RS", "Serbia"], ["SE", "Sweden"], ["SG", "Singapore"], ["SK", "Slovakia"],
  ["TH", "Thailand"], ["TR", "Turkey"], ["TW", "Taiwan"], ["UA", "Ukraine"],
  ["US", "United States"], ["VN", "Vietnam"], ["ZA", "South Africa"],
];

const MODES: Record<ConnectionProfile["psiphon_mode"], string> = {
  auto: "Automatic shape",
  cdn: "Fronted meek only (CDN)",
  direct: "Direct (no fronting)",
};

export function PsiphonOptions() {
  const status = useConnectionStore((s) => s.status);
  const extra = useConnectionStore((s) => s.profile.extra_transport);
  const region = useConnectionStore((s) => s.profile.psiphon_region);
  const setRegion = useConnectionStore((s) => s.setPsiphonRegion);
  const mode = useConnectionStore((s) => s.profile.psiphon_mode);
  const setMode = useConnectionStore((s) => s.setPsiphonMode);

  const locked = status.state !== "Idle" && status.state !== "Error";
  const isPsiphon =
    extra === "psiphon" || extra === "psiphon_reverse" || extra === "psiphon_only";
  const disabled = locked || !isPsiphon;

  const trigger =
    "w-full border-transparent bg-transparent text-muted-foreground shadow-none hover:bg-surface-2";

  return (
    <div className="flex flex-col gap-1.5 pt-1">
      <Select value={region} onValueChange={setRegion} disabled={disabled}>
        <SelectTrigger size="sm" className={trigger} aria-label="Psiphon exit region">
          <SelectValue placeholder="Exit region" />
        </SelectTrigger>
        <SelectContent>
          {REGIONS.map(([code, name]) => (
            <SelectItem key={code} value={code}>
              {code === "" ? name : `${name} (${code})`}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <Select
        value={mode}
        onValueChange={(v) => setMode(v as ConnectionProfile["psiphon_mode"])}
        disabled={disabled}
      >
        <SelectTrigger size="sm" className={trigger} aria-label="Psiphon shape">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {(Object.keys(MODES) as ConnectionProfile["psiphon_mode"][]).map((m) => (
            <SelectItem key={m} value={m}>
              {MODES[m]}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}
