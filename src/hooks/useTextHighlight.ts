import { useLayoutEffect, type RefObject } from "react";

type HighlightRegistry = Map<string, unknown>;
type HighlightConstructor = new (...ranges: Range[]) => unknown;

/** Paints every occurrence of `terms` inside the elements matching
 *  `selector` with the CSS Custom Highlight API, so the markup stays as it
 *  is. The page styles the result with `::highlight(<name>)`. */
export const useTextHighlight = (
  root: RefObject<HTMLElement>,
  name: string,
  terms: string[],
  selector: string,
) => {
  useLayoutEffect(() => {
    const registry = (CSS as unknown as { highlights?: HighlightRegistry })
      .highlights;
    const Highlight = (
      window as unknown as { Highlight?: HighlightConstructor }
    ).Highlight;
    if (!registry || !Highlight) return;
    registry.delete(name);
    const needles = terms.map((t) => t.toLowerCase()).filter(Boolean);
    if (needles.length === 0 || !root.current) return;

    const ranges: Range[] = [];
    root.current.querySelectorAll(selector).forEach((el) => {
      const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
      for (let node = walker.nextNode(); node; node = walker.nextNode()) {
        const text = node.textContent?.toLowerCase() ?? "";
        for (const needle of needles) {
          for (
            let i = text.indexOf(needle);
            i >= 0;
            i = text.indexOf(needle, i + needle.length)
          ) {
            const range = new Range();
            range.setStart(node, i);
            range.setEnd(node, i + needle.length);
            ranges.push(range);
          }
        }
      }
    });
    registry.set(name, new Highlight(...ranges));
    return () => {
      registry.delete(name);
    };
  });
};
