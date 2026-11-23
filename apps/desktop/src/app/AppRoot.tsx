import { CSPProvider } from "@base-ui/react/csp-provider";

/**
 * Wraps the whole app. `style-src 'self'` blocks inline <style> elements, so Base UI renders none;
 * the one rule it would inject (`.base-ui-disable-scrollbar`) is in index.css.
 */
export function AppRoot({ children }: { children: React.ReactNode }) {
  return <CSPProvider disableStyleElements>{children}</CSPProvider>;
}
