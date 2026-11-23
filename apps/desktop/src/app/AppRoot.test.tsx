import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectItemText,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { AppRoot } from "./AppRoot";

afterEach(cleanup);

function OpenSelect() {
  return (
    <Select<string> items={[{ value: "a", label: "A" }]} defaultValue="a" defaultOpen>
      <SelectTrigger>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="a">
          <SelectItemText>A</SelectItemText>
        </SelectItem>
      </SelectContent>
    </Select>
  );
}

const styleElements = () => document.querySelectorAll("style").length;

describe("AppRoot", () => {
  // The production CSP (`style-src 'self'`) blocks inline <style> elements in the WebView.
  it("an open select inside the app root adds no <style> element", async () => {
    const before = styleElements();
    render(
      <AppRoot>
        <OpenSelect />
      </AppRoot>,
    );
    await screen.findByRole("option", { name: "A" });
    expect(styleElements()).toBe(before);
  });

  it("control: without it the same select adds one", async () => {
    const before = styleElements();
    render(<OpenSelect />);
    await screen.findByRole("option", { name: "A" });
    expect(styleElements()).toBeGreaterThan(before);
  });
});
