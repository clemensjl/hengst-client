import { useEffect, useState, type ReactNode } from "react";

/** Shows its children only after `ms` (DESIGN.md: skeletons appear if loading takes over 200 ms). */
export function Delayed({ ms = 200, children }: { ms?: number; children: ReactNode }) {
  const [shown, setShown] = useState(false);
  useEffect(() => {
    const id = setTimeout(() => setShown(true), ms);
    return () => clearTimeout(id);
  }, [ms]);
  return shown ? children : null;
}
