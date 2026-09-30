import { useWindowFocused } from "@/state/windowFocus";

/**
 * Chic mesh backdrop: 3 drifting gradient orbs (brand orange, teal, violet)
 * + perspective grid + film-grain noise + vignette. All motion stays pure
 * CSS on compositor-promoted layers (transform/opacity only) and pauses
 * while unfocused — same perf contract as before, richer look.
 */
export function AmbientBackground() {
  const focused = useWindowFocused();
  // Inline, not a Tailwind pause class — the unlayered .anim-* shorthands
  // beat layered utilities in the cascade (see ConnectButton).
  const playState = { animationPlayState: focused ? ("running" as const) : ("paused" as const) };

  return (
    <div className="pointer-events-none absolute inset-0 z-0 overflow-hidden">
      <div
        className="anim-orb-a absolute size-70 rounded-full"
        style={{
          top: -70,
          right: -70,
          opacity: 0.2,
          background: "radial-gradient(circle, var(--color-primary) 0%, transparent 70%)",
          willChange: "transform, opacity",
          ...playState,
        }}
      />
      <div
        className="anim-orb-b absolute size-60 rounded-full"
        style={{
          bottom: -50,
          left: -90,
          opacity: 0.14,
          background:
            "radial-gradient(circle, var(--color-status-connected) 0%, transparent 70%)",
          willChange: "transform, opacity",
          ...playState,
        }}
      />
      <div
        className="anim-orb-a absolute size-52 rounded-full"
        style={{
          top: "38%",
          left: "55%",
          opacity: 0.1,
          background: "radial-gradient(circle, #8b5cf6 0%, transparent 70%)",
          willChange: "transform, opacity",
          animationDelay: "-5s",
          ...playState,
        }}
      />
      <div className="grid-overlay absolute inset-0" />
      <div className="noise-overlay absolute inset-0" />
      <div className="vignette absolute inset-0" />
    </div>
  );
}
