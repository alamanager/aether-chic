import { useEffect } from "react";
import { AnimatePresence, motion, MotionConfig } from "motion/react";
import { ShieldCheck } from "lucide-react";
import { ConnectButton } from "@/components/ConnectButton";
import { ConnectionStatusLine } from "@/components/ConnectionStatusLine";
import { TorStallGuard } from "@/components/TorStallGuard";
import { ConnectionInfo } from "@/components/ConnectionInfo";
import { AdvancedPanel } from "@/components/AdvancedPanel";
import { CloseToTrayToggle } from "@/components/CloseToTrayToggle";
import { AmbientBackground } from "@/components/AmbientBackground";
import { SidecarErrorScreen } from "@/components/SidecarErrorScreen";
import { AccessCodePrompt } from "@/components/AccessCodePrompt";
import { TooltipProvider } from "@/components/ui/tooltip";
import { TitleBar } from "@/components/TitleBar";
import { initConnectionListeners, useConnectionStore } from "@/state/connectionStore";

const SCREEN_TRANSITION = {
  initial: { opacity: 0, y: 8 },
  animate: { opacity: 1, y: 0 },
  exit: { opacity: 0, y: -4 },
  transition: { duration: 0.16, ease: [0.22, 1, 0.36, 1] as const },
};

function MainScreen() {
  const attemptId = useConnectionStore((s) => s.attemptId);
  return (
    <div className="relative z-10 flex h-full flex-col items-center gap-5 overflow-y-auto px-6 pt-2 pb-6">
      <div className="flex flex-col items-center gap-1.5 text-center">
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
      <AdvancedPanel />
      <div className="w-full max-w-sm">
        <CloseToTrayToggle />
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
