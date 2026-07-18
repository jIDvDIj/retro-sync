import { useCallback, useEffect, useState } from "react";

export type Theme = "dark" | "light";

const STORAGE_KEY = "rs-theme";

function readStoredTheme(): Theme {
  const stored = localStorage.getItem(STORAGE_KEY);
  return stored === "light" ? "light" : "dark";
}

/**
 * Preferência de tema claro/escuro — puramente de exibição, então vive só no
 * frontend (localStorage), sem round-trip pelo backend.
 */
export function useTheme() {
  const [theme, setTheme] = useState<Theme>(() => readStoredTheme());

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    localStorage.setItem(STORAGE_KEY, theme);
  }, [theme]);

  const toggle = useCallback(() => {
    setTheme((cur) => (cur === "dark" ? "light" : "dark"));
  }, []);

  return { theme, toggle };
}
