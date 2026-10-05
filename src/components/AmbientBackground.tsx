import { useWindowFocused } from "@/state/windowFocus";

/** Aurora backdrop: three drifting neon blobs (cyan/indigo/violet) over the
 * grid + noise + vignette layers. Loops are CSS-only and pause unfocused. */
export function AmbientBackground() {
  const focused = useWindowFocused();
  const playState = { animationPlayState: focused ? ("running" as const) : ("paused" as const) };

  return (
    <div aria-hidden className="pointer-events-none absolute inset-0 overflow-hidden">
      <div className="anim-orb-a absolute -top-24 -left-24 size-80 rounded-full bg-cyan-400/16 blur-3xl" style={playState} />
      <div className="anim-orb-b absolute top-1/3 -right-28 size-96 rounded-full bg-violet-500/16 blur-3xl" style={playState} />
      <div className="anim-orb-a absolute -bottom-32 left-1/4 size-72 rounded-full bg-indigo-500/14 blur-3xl" style={{ ...playState, animationDelay: "-5s" }} />
      <div className="grid-overlay absolute inset-0" />
      <div className="noise-overlay absolute inset-0" />
      <div className="vignette absolute inset-0" />
    </div>
  );
}
