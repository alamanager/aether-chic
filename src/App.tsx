import { useEffect, useState } from "react";
import { AnimatePresence, motion, MotionConfig } from "motion/react";
import { Activity, ScrollText, SlidersHorizontal, Zap } from "lucide-react";
import { ConnectButton } from "@/components/ConnectButton";
import { ConnectionStatusLine } from "@/components/ConnectionStatusLine";
import { ConnectionInfo } from "@/components/ConnectionInfo";
import { StatsTab } from "@/components/StatsTab";
import { SettingsTab } from "@/components/SettingsTab";
import { LogsTab } from "@/components/LogsTab";
import { CloseToTrayToggle } from "@/components/CloseToTrayToggle";
import { AmbientBackground } from "@/components/AmbientBackground";
import { SidecarErrorScreen } from "@/components/SidecarErrorScreen";
import { AccessCodePrompt } from "@/components/AccessCodePrompt";
import { TooltipProvider } from "@/components/ui/tooltip";
import { TitleBar } from "@/components/TitleBar";
import { cn } from "@/lib/utils";
import { initConnectionListeners, useConnectionStore } from "@/state/connectionStore";

const SCREEN_TRANSITION = {
  initial: { opacity: 0, y: 8 },
  animate: { opacity: 1, y: 0 },
  exit: { opacity: 0, y: -4 },
  transition: { duration: 0.16, ease: [0.22, 1, 0.36, 1] as const },
};

type Tab = "pulse" | "stats" | "tune" | "logs";

const TABS: { id: Tab; label: string; icon: typeof Zap }[] = [
  { id: "pulse", label: "Pulse", icon: Zap },
  { id: "stats", label: "Stats", icon: Activity },
  { id: "tune", label: "Tune", icon: SlidersHorizontal },
  { id: "logs", label: "Logs", icon: ScrollText },
];

function PulseTab() {
  const attemptId = useConnectionStore((s) => s.attemptId);
  return (
    <div className="flex flex-col items-center gap-4">
      <div className="flex flex-col items-center gap-1 text-center">
        <h1 className="text-lg font-extrabold tracking-tight">
          Ride the <span className="brand-text">aurora</span>
        </h1>
        <p className="text-[11px] text-muted-foreground">One tap between you and the open net</p>
      </div>
      <ConnectButton />
      <ConnectionStatusLine />
      <AccessCodePrompt key={attemptId} />
      <ConnectionInfo />
      <div className="w-full max-w-sm">
        <CloseToTrayToggle />
      </div>
    </div>
  );
}

function MainScreen() {
  const [tab, setTab] = useState<Tab>("pulse");
  const status = useConnectionStore((s) => s.status);
  const connected = status.state === "Connected";

  return (
    <div className="relative z-10 flex h-full flex-col overflow-hidden">
      <div className="flex min-h-0 flex-1 flex-col items-center overflow-y-auto px-6 pt-3 pb-4">
        <AnimatePresence mode="wait">
          <motion.div
            key={tab}
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -6 }}
            transition={{ duration: 0.12 }}
            className="flex w-full flex-col items-center"
          >
            {tab === "pulse" && <PulseTab />}
            {tab === "stats" && <StatsTab />}
            {tab === "tune" && <SettingsTab />}
            {tab === "logs" && <LogsTab />}
          </motion.div>
        </AnimatePresence>
      </div>
      <nav aria-label="Sections" className="flex shrink-0 justify-center px-6 pt-1 pb-4">
        <div className="glass flex items-center gap-1 rounded-2xl p-1.5">
          {TABS.map(({ id, label, icon: Icon }) => {
            const active = tab === id;
            return (
              <button
                key={id}
                type="button"
                onClick={() => setTab(id)}
                aria-pressed={active}
                className={cn(
                  "flex min-h-11 min-w-17 items-center justify-center gap-1.5 rounded-xl px-3.5 text-xs font-bold transition-all outline-none focus-visible:ring-2 focus-visible:ring-primary",
                  active
                    ? "text-white shadow-[0_4px_24px_-4px_rgb(139_92_246/0.7)]"
                    : "text-muted-foreground hover:bg-surface-2 hover:text-foreground",
                )}
                style={active ? { background: "linear-gradient(135deg, #22d3ee, #818cf8 55%, #a855f7)" } : undefined}
              >
                <Icon className="size-3.5" />
                {label}
                {id === "pulse" && connected && (
                  <span aria-hidden className={cn("size-1.5 rounded-full", active ? "bg-white" : "bg-status-connected")} />
                )}
              </button>
            );
          })}
        </div>
      </nav>
    </div>
  );
}

export function App() {
  const sidecarError = useConnectionStore((s) => s.sidecarError);
  const retryAfterSidecarError = useConnectionStore((s) => s.retryAfterSidecarError);
  const connect = useConnectionStore((s) => s.connect);

  useEffect(() => {
    const cleanup = initConnectionListeners();
    return () => {
      void cleanup.then((unlisten) => unlisten());
    };
  }, []);

  return (
    <TooltipProvider>
      <MotionConfig reducedMotion="user">
        <div className="relative flex h-svh w-full flex-col overflow-hidden bg-background">
          <AmbientBackground />
          <TitleBar />
          <div className="topline-gradient h-px w-full shrink-0" aria-hidden />
          <div className="relative min-h-0 flex-1">
            <AnimatePresence mode="sync">
              {sidecarError ? (
                <motion.div key="error" className="absolute inset-0 z-10" {...SCREEN_TRANSITION}>
                  <SidecarErrorScreen
                    message={sidecarError}
                    onRetry={() => {
                      retryAfterSidecarError();
                      void connect();
                    }}
                  />
                </motion.div>
              ) : (
                <motion.div key="main" className="absolute inset-0" {...SCREEN_TRANSITION}>
                  <MainScreen />
                </motion.div>
              )}
            </AnimatePresence>
          </div>
        </div>
      </MotionConfig>
    </TooltipProvider>
  );
}

export default App;
