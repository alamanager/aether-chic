import { useCallback, useEffect, useState } from "react";
import { Moon, Sun } from "lucide-react";

const KEY = "aether-theme"; // "dark" | "light"

function currentTheme(): "dark" | "light" {
  if (typeof document !== "undefined" && document.documentElement.classList.contains("light"))
    return "light";
  try {
    return localStorage.getItem(KEY) === "light" ? "light" : "dark";
  } catch {
    return "dark";
  }
}

export function ThemeToggle() {
  const [theme, setTheme] = useState<"dark" | "light">(currentTheme);

  useEffect(() => {
    document.documentElement.classList.toggle("light", theme === "light");
    try {
      localStorage.setItem(KEY, theme);
    } catch {
      /* private mode — theme just won't persist */
    }
  }, [theme]);

  const toggle = useCallback(
    () => setTheme((t) => (t === "dark" ? "light" : "dark")),
    [],
  );

  return (
    <button
      type="button"
      onClick={toggle}
      aria-label={theme === "dark" ? "Switch to light theme" : "Switch to dark theme"}
      title={theme === "dark" ? "Light theme" : "Dark theme"}
      className="grid size-11 place-items-center rounded-full text-muted-foreground transition-colors outline-none hover:bg-surface-2 hover:text-foreground focus-visible:ring-2 focus-visible:ring-primary"
    >
      {theme === "dark" ? <Sun className="size-4.5" /> : <Moon className="size-4.5" />}
    </button>
  );
}
