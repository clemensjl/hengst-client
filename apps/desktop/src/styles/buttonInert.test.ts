import { describe, expect, it } from "vitest";
import { buttonVariants } from "@/components/ui/button";

// An inert button (`aria-disabled`, focusable, its reason as description) does nothing, so it
// shows neither the hover nor the pressed state of its variant.
describe("inert buttons", () => {
  it.each([
    ["primary", "aria-disabled:hover:bg-primary", "aria-disabled:active:bg-primary"],
    ["secondary", "aria-disabled:hover:bg-secondary", "aria-disabled:active:bg-secondary"],
    ["ghost", "aria-disabled:hover:bg-transparent", "aria-disabled:active:bg-transparent"],
    ["destructive", "aria-disabled:hover:bg-destructive", "aria-disabled:active:bg-destructive"],
    ["link", "aria-disabled:hover:decoration-1", "aria-disabled:active:text-foreground"],
  ] as const)("%s keeps its rest look on hover and press", (variant, hover, active) => {
    const classes = buttonVariants({ variant }).split(" ");
    expect(classes).toContain(hover);
    expect(classes).toContain(active);
  });
});
