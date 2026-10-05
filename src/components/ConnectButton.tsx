import { AnimatePresence, motion, type Variants } from "motion/react";
import { AlertTriangle, Check, Loader2, Power } from "lucide-react";
import { cn } from "@/lib/utils";
import { useConnectionStore } from "@/state/connectionStore";
import { useWindowFocused } from "@/state/windowFocus";
import type { ConnectionStatus } from "@/types/connection";

type Phase = "idle" | "connecting" | "connected" | "error";

function phaseOf(status: ConnectionStatus): Phase {
  switch (status.state) {
    case "Launching":
    case "Connecting":
    case "Reconnecting":
    case "Disconnecting":
      return "connecting";
    case "Connected":
      return "connected";
    case "Error":
      return "error";
    default:
      return "idle";
  }
}

/* Motion: one-shots only (tap, error shake, icon swap). All loops are CSS. */
const SHAKE: Variants = {
  rest: { x: 0 },
  error: { x: [0, -6, 6, -4, 4, 0], transition: { x: { duration: 0.4, ease: "easeInOut" } } },
};

/** Aurora conic per phase: idle slate shimmer, working violet storm,
 * connected emerald crown, error red. */
const HALO: Record<Phase, string> = {
  idle: "conic-gradient(from 0deg, #3b4266, #22d3ee55, #818cf855, #a855f755, #3b4266)",
  connecting:
    "conic-gradient(from 0deg, #a855f7, #22d3ee, #818cf8, #a855f7)",
  connected:
    "conic-gradient(from 0deg, #22c55e, #22d3ee, #22c55e)",
  error:
    "conic-gradient(from 0deg, var(--color-status-error), transparent 40%, var(--color-status-error))",
};

const GLOW: Record<Phase, string> = {
  idle: "0 0 0 1px rgb(255 255 255 / 0.08), 0 20px 60px -20px rgb(0 0 0 / 0.9)",
  connecting:
    "0 0 0 1px rgb(168 85 247 / 0.45), 0 0 60px -8px rgb(139 92 246 / 0.55)",
  connected:
    "0 0 0 1px rgb(34 197 94 / 0.4), 0 0 70px -10px rgb(34 197 94 / 0.5)",
  error:
    "0 0 0 1px rgb(248 113 113 / 0.5), 0 0 44px -8px rgb(248 113 113 / 0.5)",
};

const ICONS: Record<Phase, typeof Power> = {
  idle: Power,
  connecting: Loader2,
  connected: Check,
  error: AlertTriangle,
};

const ARIA_LABEL: Record<Phase, string> = {
  idle: "Connect",
  connecting: "Cancel connecting",
  connected: "Disconnect",
  error: "Retry connection",
};

export function ConnectButton() {
  const status = useConnectionStore((s) => s.status);
  const connect = useConnectionStore((s) => s.connect);
  const disconnect = useConnectionStore((s) => s.disconnect);
  const focused = useWindowFocused();

  const phase = phaseOf(status);
  const Icon = ICONS[phase];
  const playState = { animationPlayState: focused ? ("running" as const) : ("paused" as const) };

  const handleClick = () => {
    if (phase === "idle" || phase === "error") void connect();
    else void disconnect();
  };

  return (
    <motion.button
      type="button"
      aria-label={ARIA_LABEL[phase]}
      onClick={handleClick}
      disabled={status.state === "Disconnecting"}
      whileTap={{ scale: 0.95 }}
      whileHover={{ scale: 1.02 }}
      animate={phase === "error" ? "error" : "rest"}
      variants={SHAKE}
      className="relative flex size-44 items-center justify-center rounded-full text-foreground outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2 focus-visible:ring-offset-background motion-reduce:transition-none"
      style={playState}
    >
      <span
        aria-hidden
        className={cn(
          "anim-aurora-spin absolute inset-0 rounded-full",
          phase === "connecting" && "[animation-duration:1.6s]",
        )}
        style={{ background: HALO[phase], willChange: "transform", ...playState }}
      />
      <span
        aria-hidden
        className={cn(
          "absolute inset-[4px] rounded-full backdrop-blur-xl",
          phase === "idle" && "anim-ring-breathe",
          phase === "connecting" && "anim-ring-pulse-fast",
          phase === "connected" && "anim-ring-pulse-slow",
        )}
        style={{
          background: "color-mix(in oklch, var(--color-surface-2) 86%, transparent)",
          boxShadow: GLOW[phase],
          transition: "box-shadow 0.25s ease",
          willChange: "transform, opacity",
          ...playState,
        }}
      />
      <span
        aria-hidden
        className="pointer-events-none absolute inset-[4px] rounded-full"
        style={{
          background:
            "radial-gradient(circle at 50% 26%, rgb(255 255 255 / 0.16), transparent 55%)",
        }}
      />
      <AnimatePresence mode="wait">
        <motion.span
          key={phase}
          initial={{ opacity: 0, scale: 0.8 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.8 }}
          transition={{ duration: 0.1, ease: [0.4, 0, 0.2, 1] }}
          className="relative flex items-center justify-center"
        >
          <Icon
            size={54}
            strokeWidth={1.8}
            style={phase === "connecting" ? playState : undefined}
            className={
              phase === "connecting"
                ? "animate-spin text-[#c4b5fd]"
                : phase === "connected"
                  ? "text-status-connected"
                  : phase === "error"
                    ? "text-status-error"
                    : "text-[#a5b4fc]"
            }
          />
        </motion.span>
      </AnimatePresence>
    </motion.button>
  );
}
