import { getCurrentWindow } from "@tauri-apps/api/window";
import { Maximize2, Minus, X } from "lucide-react";
import { ThemeToggle } from "@/components/ThemeToggle";

const appWindow = getCurrentWindow();

export function TitleBar() {
  return (
    // data-tauri-drag-region only fires when the mousedown target IS this
    // element, so the buttons stay clickable without any extra handling.
    <header
      data-tauri-drag-region
      className="relative z-10 flex h-12 shrink-0 select-none items-center gap-2 px-3"
    >
      <div data-tauri-drag-region className="flex min-w-0 flex-1 items-center gap-2.5">
        <span
          aria-hidden
          className="grid size-7 shrink-0 place-items-center rounded-xl text-sm font-bold"
          style={{
            background: "linear-gradient(135deg, var(--color-primary), #b43c02)",
            color: "#0d0d0f",
          }}
        >
          Æ
        </span>
        <span data-tauri-drag-region className="truncate text-[13px] font-bold tracking-[0.18em]">
          AETHER
        </span>
        <span className="hidden rounded-full bg-surface-2 px-2 py-0.5 font-mono text-[10px] text-muted-foreground sm:inline">
          v0.7
        </span>
      </div>
      <ThemeToggle />
      <div className="flex h-full items-center">
        <button
          aria-label="Minimize"
          className="grid h-full w-11 place-items-center rounded-md text-muted-foreground hover:bg-surface-2 hover:text-foreground"
          onClick={() => void appWindow.minimize()}
        >
          <Minus className="size-4" />
        </button>
        <button
          aria-label="Maximize"
          className="grid h-full w-11 place-items-center rounded-md text-muted-foreground hover:bg-surface-2 hover:text-foreground"
          onClick={() => void appWindow.toggleMaximize()}
        >
          <Maximize2 className="size-3.5" />
        </button>
        <button
          aria-label="Close"
          className="grid h-full w-11 place-items-center rounded-md text-muted-foreground hover:bg-destructive hover:text-white"
          onClick={() => void appWindow.close()}
        >
          <X className="size-4" />
        </button>
      </div>
    </header>
  );
}
