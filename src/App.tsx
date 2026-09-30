import { useEffect, useState } from "react";
import { AnimatePresence, motion, MotionConfig } from "motion/react";
import { House, ScrollText, Settings2, ShieldCheck } from "lucide-react";
import { ConnectButton } from "@/components/ConnectButton";
import { ConnectionStatusLine } from "@/components/ConnectionStatusLine";
import { TorStallGuard } from "@/components/TorStallGuard";
import { ConnectionInfo } from "@/components/ConnectionInfo";
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

type Tab = "home" | "settings" | "logs";

const TABS: { id: Tab; label: string; icon: typeof House }[] = [
  { id: "home", label: "Home", icon: House },
  { id: "settings", label: "Settings", icon: Settings2 },
  { id: "logs", label: "Logs", icon: ScrollText },
];

function HomeTab() {
  const attemptId = useConnectionStore((s) => s.attemptId);
  return (
    <div className="flex flex-col items-center gap-4">
      <div className="flex h-[64px] flex-col items-center justify-center gap-1.5 text-center">
        <span className="glass flex items-center gap-1.5 rounded-full px-3 py-1 text-[11px] font-medium text-muted-foreground">
          <ShieldCheck className="size-3.5 text-status-connected" />
          Censorship-circumvention tunnel
        </span>
        <h1 className="text-xl font-bold tracking-tight">
          One tap to <span className="brand-text">free internet</span>
        </h1>
      </div>
      <ConnectButton />
      <ConnectionStatusLine />
      <TorStallGuard key={attemptId} />
      <AccessCodePrompt key={attemptId} />
      <ConnectionInfo />
      <div className="w-full max-w-sm">
        <CloseToTrayToggle />
      </div>
    </div>
  );
}

function MainScreen() {
  const [tab, setTab] = useState<Tab>("home");
  const status = useConnectionStore((s) => s.status);
  const connected = status.state === "Connected";

  return (
    <div className="relative z-10 flex h-full flex-col items-center overflow-y-auto px-6 pt-2 pb-6">
      <nav
        aria-label="Sections"
        className="glass mb-4 flex shrink-0 items-center gap-1 rounded-2xl p-1.5"
      >
        {TABS.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            type="button"
            onClick={() => setTab(id)}
            aria-pressed={tab === id}
            className={cn(
              "flex min-h-11 items-center gap-1.5 rounded-xl px-4 text-xs font-semibold transition-all",
              tab === id
                ? "bg-primary text-primary-foreground shadow-[0_4px_20px_-4px_var(--color-primary)]"
                : "text-muted-foreground hover:bg-surface-2 hover:text-foreground",
            )}
          >
            <Icon className="size-3.5" />
            {label}
            {id === "home" && connected && (
              <span
                aria-hidden
                className={cn(
                  "size-1.5 rounded-full",
                  tab === id ? "bg-primary-foreground" : "bg-status-connected",
                )}
              />
            )}
          </button>
        ))}
      </nav>
      <div className="flex w-full flex-1 flex-col items-center">
        <AnimatePresence mode="wait">
          <motion.div
            key={tab}
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -6 }}
            transition={{ duration: 0.12 }}
            className="flex w-full flex-col items-center"
          >
            {tab === "home" && <HomeTab />}
            {tab === "settings" && <SettingsTab />}
            {tab === "logs" && <LogsTab />}
          </motion.div>
        </AnimatePresence>
      </div>
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
