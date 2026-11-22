import type { Plugin } from "vite";

/**
 * The dev-only galleries (`src/dev/ErrorGallery.tsx`, `src/dev/SignInGallery.tsx`) must not be in a production bundle. Its
 * name, its hash and its sample text are searched for in the output of every build; a hit fails the
 * build, so the check does not depend on the order of the CI steps.
 */
export const GALLERY_MARKERS = [
  "error-gallery",
  "ErrorGallery",
  "gallery-sample-detail",
  "sign-in-gallery",
  "SignInGallery",
  "gallery sample:",
] as const;

/** `file: marker` for every marker found in the named outputs. */
export function findGalleryMarkers(outputs: Record<string, string>): string[] {
  return Object.entries(outputs).flatMap(([file, text]) =>
    GALLERY_MARKERS.filter((marker) => text.includes(marker)).map((marker) => `${file}: ${marker}`),
  );
}

export function noDevGallery(): Plugin {
  return {
    name: "no-dev-gallery",
    apply: "build",
    enforce: "post",
    generateBundle(_options, bundle) {
      const outputs: Record<string, string> = {};
      for (const [name, item] of Object.entries(bundle)) {
        if (item.type === "chunk") outputs[name] = item.code;
        else if (typeof item.source === "string") outputs[name] = item.source;
      }
      const hits = findGalleryMarkers(outputs);
      if (hits.length > 0) {
        this.error(
          "a dev-only gallery is part of the production build (it must stay behind " +
            `import.meta.env.DEV): ${hits.join(", ")}`,
        );
      }
    },
  };
}
