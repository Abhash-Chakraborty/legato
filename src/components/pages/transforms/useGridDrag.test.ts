import assert from "node:assert/strict";
import { nearestSlot, reorder } from "./useGridDrag";

assert.deepEqual(reorder(["a", "b", "c", "d"], 0, 2), ["b", "c", "a", "d"]);
assert.deepEqual(reorder(["a", "b", "c", "d"], 3, 1), ["a", "d", "b", "c"]);
assert.deepEqual(reorder(["a", "b"], 1, 1), ["a", "b"]);

// Two columns, two rows of 100 × 50 cards.
const slot = (left: number, top: number) =>
  ({ left, top, width: 100, height: 50 }) as DOMRect;
const rects = [slot(0, 0), slot(110, 0), slot(0, 60), slot(110, 60)];
assert.equal(nearestSlot(rects, 50, 25), 0);
assert.equal(nearestSlot(rects, 150, 90), 3);
assert.equal(nearestSlot(rects, 40, 200), 2);

console.log("grid drag: ok");
