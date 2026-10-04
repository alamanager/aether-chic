import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useConnectionStore } from "@/state/connectionStore";
import type { Noize } from "@/types/connection";

const LABELS: Record<Noize, string> = {
  off: "Off",
  light: "Light",
  firewall: "Firewall",
  balanced: "Balanced",
  gfw: "GFW",
  aggressive: "Aggressive",
};

const DESCRIPTIONS: Record<Noize, string> = {
  off: "No obfuscation. Only for open networks or testing.",
  light: "Gentlest disguise, least overhead.",
  firewall: "Default for MASQUE — gets through most filtered networks.",
  balanced: "Default for WireGuard — good stealth/speed balance.",
  gfw: "Heavy decoy traffic for strict censorship.",
  aggressive: "Heaviest obfuscation. Slowest, for the toughest networks.",
};

/** One list for every protocol since core v2.2. Locked outside Idle/Error. */
export function NoizeProfileToggle() {
  const status = useConnectionStore((s) => s.status);
  const noize = useConnectionStore((s) => s.profile.noize);
  const setNoize = useConnectionStore((s) => s.setNoize);

  const locked = status.state !== "Idle" && status.state !== "Error";

  return (
    <ToggleGroup
      type="single"
      value={noize}
      onValueChange={(v) => {
        if (v) setNoize(v as Noize);
      }}
      disabled={locked}
      className="w-full flex-wrap gap-1 rounded-2xl bg-black/20 p-1 ring-1 ring-white/10"
    >
      {(Object.keys(LABELS) as Noize[]).map((n) => (
        <Tooltip key={n}>
          {/* asChild targets this plain span, not ToggleGroupItem directly —
           * Radix's Slot cloning onto ToggleGroupItem's own internals was
           * silently breaking its data-state/pressed rendering. */}
          <TooltipTrigger asChild>
            <span className="flex-1">
              <ToggleGroupItem
                value={n}
                size="sm"
                aria-label={LABELS[n]}
                className="w-full rounded-full text-muted-foreground transition-colors duration-75 data-[state=on]:bg-primary/85 data-[state=on]:text-primary-foreground"
              >
                {LABELS[n]}
              </ToggleGroupItem>
            </span>
          </TooltipTrigger>
          <TooltipContent>{DESCRIPTIONS[n]}</TooltipContent>
        </Tooltip>
      ))}
    </ToggleGroup>
  );
}
