/**
 * Tells CSS how the last input came in: `data-input="pointer"` after a pointer-down, `"keyboard"`
 * after a key press. With `keyboard`, layers that open appear without transition (DESIGN.md 8 and
 * Appendix A).
 */
export function installInputMode(): () => void {
  const root = document.documentElement;
  const pointer = () => {
    root.dataset.input = "pointer";
  };
  const keyboard = () => {
    root.dataset.input = "keyboard";
  };
  document.addEventListener("pointerdown", pointer, true);
  document.addEventListener("keydown", keyboard, true);
  return () => {
    document.removeEventListener("pointerdown", pointer, true);
    document.removeEventListener("keydown", keyboard, true);
  };
}
