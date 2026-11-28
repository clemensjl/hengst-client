// Test helper: what a screen reader would announce. A live region announces text that changes
// inside it after it is in the document; a region that mounts together with its text announces
// nothing. MutationObserver tells the two apart: text added inside an existing region is a
// mutation whose target is in that region, a region inserted with its text is a mutation of its
// parent.

const REGION = '[aria-live], [role="status"], [role="alert"]';

/** Starts recording; `announced()` returns the texts of the regions that changed so far. */
export function watchAnnouncements(root: Node = document.body) {
  const seen: string[] = [];
  const take = (records: MutationRecord[]) => {
    for (const r of records) {
      const el = r.target instanceof Element ? r.target : r.target.parentElement;
      const region = el?.closest(REGION);
      if (region?.textContent) seen.push(region.textContent);
    }
  };
  const observer = new MutationObserver(take);
  observer.observe(root, { childList: true, subtree: true, characterData: true });
  return {
    announced: () => {
      take(observer.takeRecords());
      return seen;
    },
    stop: () => observer.disconnect(),
  };
}
