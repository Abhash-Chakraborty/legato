/** Scrolls an opened popover or menu just far enough to show all of it, so
 *  one opened near the bottom of a list doesn't hang off screen. */
export const reveal = (el: HTMLElement) => {
  const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
  el.scrollIntoView({ block: "nearest", behavior: still ? "auto" : "smooth" });
};
