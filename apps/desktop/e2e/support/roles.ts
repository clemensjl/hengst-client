/**
 * The one displayed element with this role and accessible name, or `null` when there is none or
 * more than one. Covers explicit roles and the implicit ones the specs need: `heading` (h1 to h6),
 * `button`, `checkbox`, `textbox`. The name is computed as in `focusedName`.
 *
 * Skipped, as the accessibility tree skips them: anything inside an `aria-hidden="true"` subtree
 * (Base UI's checkbox puts a hidden `<input type="checkbox" aria-hidden>` beside the real one) and
 * an element whose explicit role is another one (a `<button role="checkbox">` is no button).
 *
 * It runs in the page (`browser.execute`) and in unit tests, so it must stay self-contained: it
 * reads nothing from outside its own body. `layout` false leaves out the visibility check, for a
 * DOM without layout (jsdom).
 */
export function singleByRole(
  wantedRole: string,
  wantedName: string,
  layout = true,
): Element | null {
  const implicit: Record<string, string> = {
    heading: "h1,h2,h3,h4,h5,h6",
    button: "button",
    checkbox: 'input[type="checkbox"]',
    textbox: 'input:not([type]),input[type="text"],textarea',
  };
  const selector = [`[role="${wantedRole}"]`, implicit[wantedRole]].filter(Boolean).join(",");
  const text = (node: Element | null) => (node?.textContent ?? "").replace(/\s+/g, " ").trim();
  const nameOf = (el: Element) => {
    const label = el.getAttribute("aria-label");
    if (label) return label;
    const ids = el.getAttribute("aria-labelledby");
    if (ids) {
      return ids
        .split(/\s+/)
        .map((id) => text(document.getElementById(id)))
        .join(" ");
    }
    if (el.id) {
      const forLabel = document.querySelector(`label[for="${el.id}"]`);
      if (forLabel) return text(forLabel);
    }
    const wrapping = el.closest("label");
    return wrapping ? text(wrapping) : text(el);
  };
  const exposed = (el: Element) => {
    const explicit = el.getAttribute("role");
    return (!explicit || explicit === wantedRole) && !el.closest('[aria-hidden="true"]');
  };
  const visible = (el: Element) => !layout || el.getClientRects().length > 0;
  const all = [...document.querySelectorAll(selector)].filter(
    (el) => exposed(el) && visible(el) && nameOf(el) === wantedName,
  );
  return all.length === 1 ? all[0] : null;
}
