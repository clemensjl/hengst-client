import { render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup } from "@testing-library/react";
import { Checkbox } from "@/components/ui/checkbox";
import { singleByRole } from "../../e2e/support/roles.ts";

afterEach(() => cleanup());

/** jsdom has no layout: every element would count as hidden, so the check is left out here. */
const find = (role: string, name: string) => singleByRole(role, name, false);

describe("singleByRole", () => {
  it("finds a Base UI checkbox by its label, not its hidden input", () => {
    render(
      <label>
        <Checkbox />
        Beta
      </label>,
    );
    // The premise: Base UI renders a second, hidden checkbox input next to the visible one.
    expect(document.querySelectorAll('input[type="checkbox"]').length).toBeGreaterThan(0);
    const found = find("checkbox", "Beta");
    expect(found).not.toBeNull();
    expect(found!.getAttribute("role")).toBe("checkbox");
  });

  it("skips anything inside an aria-hidden subtree", () => {
    render(
      <>
        <button>Save</button>
        <div aria-hidden="true">
          <button>Save</button>
        </div>
      </>,
    );
    expect(find("button", "Save")?.closest('[aria-hidden="true"]')).toBeNull();
  });

  it("does not take a button with another explicit role for a button", () => {
    render(
      <>
        <button role="checkbox" aria-checked="false">
          Beta
        </button>
        <button>Beta</button>
      </>,
    );
    expect(find("button", "Beta")?.hasAttribute("role")).toBe(false);
    expect(find("checkbox", "Beta")?.getAttribute("role")).toBe("checkbox");
  });

  it("finds nothing when the name is ambiguous", () => {
    render(
      <>
        <button>Save</button>
        <button>Save</button>
      </>,
    );
    expect(find("button", "Save")).toBeNull();
  });
});
