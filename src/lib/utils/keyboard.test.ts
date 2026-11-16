import assert from "node:assert/strict";
import { formatKeyCombination, getKeyName } from "./keyboard";

const keyboardEvent = (value: { code?: string; key?: string }): KeyboardEvent =>
  value as KeyboardEvent;

const compoundKeys = [
  ["ScrollLock", "scrolllock", "Scroll Lock"],
  ["CapsLock", "capslock", "Caps Lock"],
  ["NumLock", "numlock", "Num Lock"],
  ["PageUp", "pageup", "Page Up"],
  ["PageDown", "pagedown", "Page Down"],
  ["PrintScreen", "printscreen", "Print Screen"],
] as const;

for (const [code, stored, displayed] of compoundKeys) {
  assert.equal(getKeyName(keyboardEvent({ code })), stored);
  assert.equal(formatKeyCombination(stored, false), displayed);
}

// The Windows key is stored as "super" but shown as Win.
assert.equal(getKeyName(keyboardEvent({ code: "MetaLeft" })), "super");
assert.equal(
  formatKeyCombination("super+shift+space", false),
  "Win + Shift + Space",
);
assert.equal(formatKeyCombination("super_left+v", false), "Left Win + V");
assert.equal(formatKeyCombination("ctrl+alt+w", false), "Ctrl + Alt + W");

// A Mac names its own keys, whichever way the shortcut spells them.
assert.equal(formatKeyCombination("option+space", true), "Option + Space");
assert.equal(
  formatKeyCombination("ctrl+option+w", true),
  "Control + Option + W",
);
assert.equal(
  formatKeyCombination("command_left+shift+enter", true),
  "Left Command + Shift + Return",
);
assert.equal(formatKeyCombination("super+alt+1", true), "Command + Option + 1");
assert.equal(formatKeyCombination("fn", true), "fn");

assert.equal(getKeyName(keyboardEvent({ key: "CapsLock" })), "capslock");
assert.equal(
  getKeyName(keyboardEvent({ code: "AudioVolumeUp" })),
  "audiovolumeup",
);

console.log("keyboard: all assertions passed");
