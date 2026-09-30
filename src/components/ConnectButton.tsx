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

/** Motion handles ONLY one-shots here (error shake, tap). Every infinite
 * loop is a CSS animation from index.css on a compositor-promoted layer —
 * Motion's JS-driven loops cost a style recalc every frame at 60fps, and
 * its box-shadow tweens with var()/color-mix() values never converge at all
 * (traced live: an endless per-frame write pinned the compositor forever). */
const SHAKE_VARIANTS: Variants = {
  rest: { x: 0 },
  error: { x: [0, -6, 6, -4, 4, 0], transition: { x: { duration: 0.4, ease: "easeInOut" } } },
};

/** Conic gradient per phase for the rotating halo ring. */
const HALO: Record<Phase, string> = {
  idle: "conic-gradient(from 0deg, var(--color-status-idle), color-mix(in oklch, var(--color-status-idle) 25%, transparent), var(--color-status-idle))",
  connecting:
    "conic-gradient(from 0deg, var(--color-status-connecting), var(--color-status-connected), var(--color-status-connecting))",
  connected:
    "conic-gradient(from 0deg, var(--color-status-connected), var(--color-primary), var(--color-status-connected))",
  error:
    "conic-gradient(from 0deg, var(--color-status-error), color-mix(in oklch, var(--color-status-error) 30%, transparent), var(--color-status-error))",
};

/** Soft outer shadow painted once per phase (no per-frame tween). */
const DISC_SHADOW: Record<Phase, string> = {
  idle: "0 0 0 1px color-mix(in oklch, white 8%, transparent), 0 20px 60px -20px rgb(0 0 0 / 0.8)",
  connecting:
    "0 0 0 1px color-mix(in oklch, var(--color-status-connecting) 45%, transparent), 0 0 44px -6px color-mix(in oklch, var(--color-status-connecting) 55%, transparent)",
  connected:
    "0 0 0 1px color-mix(in oklch, var(--color-status-connected) 40%, transparent), 0 0 54px -8px color-mix(in oklch, var(--color-status-connected) 60%, transparent)",
  error:
    "0 0 0 1px color-mix(in oklch, var(--color-status-error) 50%, transparent), 0 0 40px -8px color-mix(in oklch, var(--color-status-error) 55%, transparent)",
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
  // Unfocused = nobody is watching, and any running animation keeps the
  // WebView2 compositor redrawing at 60fps in the background — pause (not
  // remove) so nothing jumps on refocus. Inline style, NOT a Tailwind
  // [animation-play-state:paused] class: the .anim-* shorthands are
  // unlayered CSS and silently beat layered utilities in the cascade
  // (verified live — the class was applied yet computed state stayed
  // "running"). Reduced motion is handled in CSS.
  const playState = { animationPlayState: focused ? ("running" as const) : ("paused" as const) };

  const handleClick = () => {
    if (phase === "idle" || phase === "error") {
      void connect();
    } else {
      void disconnect();
    }
  };

  return (
    <motion.button
      type="button"
      aria-label={ARIA_LABEL[phase]}
      onClick={handleClick}
      disabled={status.state === "Disconnecting"}
      whileTap={{ scale: 0.96 }}
      whileHover={{ scale: 1.02 }}
      animate={phase === "error" ? "error" : "rest"}
      variants={SHAKE_VARIANTS}
      className="anim-float-soft relative flex size-44 items-center justify-center rounded-full text-foreground outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2 focus-visible:ring-offset-background motion-reduce:transition-none"
      style={playState}
    >
      {/* Rotating conic halo. Only this layer spins (transform-only,
        compositor-promoted); the icon above never inherits the rotation. */}
      <span
        aria-hidden
        className={cn(
          "absolute inset-0 rounded-full",
          phase === "connecting" && "animate-spin [animation-duration:1.8s]",
          phase === "connected" && "anim-spin-slower",
        )}
        style={{ background: HALO[phase], willChange: "transform", ...playState }}
      />
      {/* Glass disc covering the halo's center, leaving a 3px gradient ring.
        Breathing/pulse loops animate THIS span, never fighting Motion's
        tap/shake transforms on the button itself. */}
      <span
        aria-hidden
        className={cn(
          "absolute inset-[3px] rounded-full backdrop-blur-xl",
          phase === "idle" && "anim-ring-breathe",
          phase === "connecting" && "anim-ring-pulse-fast",
          phase === "connected" && "anim-ring-pulse-slow",
        )}
        style={{
          background:
            "color-mix(in oklch, var(--color-surface-2) 82%, transparent)",
          boxShadow: DISC_SHADOW[phase],
          transition: "box-shadow 0.2s ease",
          willChange: "transform, opacity",
          ...playState,
        }}
      />
      {/* Inner highlight for a lit-glass look */}
      <span
        aria-hidden
        className="pointer-events-none absolute inset-[3px] rounded-full"
        style={{
          background:
            "radial-gradient(circle at 50% 28%, rgb(255 255 255 / 0.14), transparent 55%)",
        }}
      />

      <AnimatePresence>
        {(phase === "connecting" || phase === "connected") && (
          <motion.span
            key={phase}
            aria-hidden
            className="pointer-events-none absolute inset-0 rounded-full border-2"
            style={{
              borderColor:
                phase === "connected"
                  ? "var(--color-status-connected)"
                  : "var(--color-status-connecting)",
            }}
            initial={{ scale: 0.9, opacity: 0.55 }}
            animate={{ scale: phase === "connected" ? 2 : 1.7, opacity: 0 }}
            transition={{ duration: phase === "connected" ? 0.9 : 0.7, ease: "easeOut" }}
          />
        )}
      </AnimatePresence>

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
            size={56}
            strokeWidth={1.8}
            style={phase === "connecting" ? playState : undefined}
            className={
              phase === "connecting"
                ? "animate-spin text-status-connecting"
                : phase === "connected"
                  ? "text-status-connected"
                  : phase === "error"
                    ? "text-status-error"
                    : "text-status-idle"
            }
          />
        </motion.span>
      </AnimatePresence>
    </motion.button>
  );
}
