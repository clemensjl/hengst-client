import { createCn } from "cn/config";

/**
 * Class merging that knows the type scale of DESIGN.md 2.5, so `text-label text-foreground` keeps
 * both the size and the color.
 */
export const cn = createCn({
  extend: {
    classGroups: {
      "font-size": [
        {
          text: [
            "display-lg",
            "display",
            "heading",
            "body",
            "label",
            "small",
            "caption",
            "badge",
            "play",
            "mono",
            "code",
          ],
        },
      ],
    },
  },
});
