import { getCurrentWindow } from "@tauri-apps/api/window";
import { Maximize2, Minus, X } from "lucide-react";
import { ThemeToggle } from "@/components/ThemeToggle";

const appWindow = getCurrentWindow();

export function TitleBar() {
  return (
    <header
      data-tauri-drag-region
      className="relative z-10 flex h-12 shrink-0 items-center gap-2 px-3 select-none"
    >
      <div data-tauri-drag-region className="flex min-w-0 flex-1 items-center gap-2.5">
        <span
          aria-hidden
          className="grid size-7 shrink-0 place-items-center rounded-lg text-sm font-black text-white shadow-[0_0_18px_-2px_var(--color-primary)]"
          style={{ background: "linear-gradient(135deg, #22d3ee, #818cf8 55%, #a855f7)" }}
        >
          Æ
        </span>
        <span data-tauri-drag-region className="truncate text-[13px] font-extrabold tracking-[0.22em]">
          AETHER
        </span>
        <span className="hidden rounded-full bg-surface-2 px-2 py-0.5 font-mono text-[10px] text-muted-foreground ring-1 ring-white/10 sm:inline">
          v0.7
        </span>
      </div>
      <ThemeToggle />
      <div className="flex h-full items-center">
        <button
          aria-label="Minimize"
          className="grid h-full w-11 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-surface-2 hover:text-foreground"
          onClick={() => void appWindow.minimize()}
        >
          <Minus className="size-4" />
        </button>
        <button
          aria-label="Maximize"
          className="grid h-full w-11 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-surface-2 hover:text-foreground"
          onClick={() => void appWindow.toggleMaximize()}
        >
          <Maximize2 className="size-3.5" />
        </button>
        <button
          aria-label="Close"
          className="grid h-full w-11 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-destructive hover:text-white"
          onClick={() => void appWindow.close()}
        >
          <X className="size-4" />
        </button>
      </div>
    </header>
  );
}
