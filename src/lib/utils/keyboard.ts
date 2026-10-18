/**
 * Keyboard utility functions for handling keyboard events
 */

// Shortcut parsers require compact tokens; keep display labels separate.
const KEY_DISPLAY: Record<string, string> = {
  capslock: "Caps Lock",
  numlock: "Num Lock",
  pageup: "Page Up",
  pagedown: "Page Down",
  printscreen: "Print Screen",
  scrolllock: "Scroll Lock",
  // The parsers call the Windows key "super".
  super: "Win",
};

/**
 * Extract a consistent key name from a KeyboardEvent
 */
export const getKeyName = (e: KeyboardEvent): string => {
  // Handle special cases first
  if (e.code) {
    const code = e.code;

    // Handle function keys (F1-F24)
    if (code.match(/^F\d+$/)) {
      return code.toLowerCase(); // F1, F2, ..., F14, F15, etc.
    }

    // Handle regular letter keys (KeyA -> a)
    if (code.match(/^Key[A-Z]$/)) {
      return code.replace("Key", "").toLowerCase();
    }

    // Handle digit keys (Digit0 -> 0)
    if (code.match(/^Digit\d$/)) {
      return code.replace("Digit", "");
    }

    // Handle numpad digit keys (Numpad0 -> numpad 0)
    if (code.match(/^Numpad\d$/)) {
      return code.replace("Numpad", "numpad ").toLowerCase();
    }

    // Handle modifier keys (the Windows key is "super")
    const modifierMap: Record<string, string> = {
      ShiftLeft: "shift",
      ShiftRight: "shift",
      ControlLeft: "ctrl",
      ControlRight: "ctrl",
      AltLeft: "alt",
      AltRight: "alt",
      MetaLeft: "super",
      MetaRight: "super",
      OSLeft: "super",
      OSRight: "super",
      CapsLock: "capslock",
      Tab: "tab",
      Enter: "enter",
      Space: "space",
      Backspace: "backspace",
      Delete: "delete",
      Escape: "esc",
      ArrowUp: "up",
      ArrowDown: "down",
      ArrowLeft: "left",
      ArrowRight: "right",
      Home: "home",
      End: "end",
      PageUp: "pageup",
      PageDown: "pagedown",
      Insert: "insert",
      PrintScreen: "printscreen",
      ScrollLock: "scrolllock",
      Pause: "pause",
      ContextMenu: "menu",
      NumpadMultiply: "numpad *",
      NumpadAdd: "numpad +",
      NumpadSubtract: "numpad -",
      NumpadDecimal: "numpad .",
      NumpadDivide: "numpad /",
      NumLock: "numlock",
    };

    if (modifierMap[code]) {
      return modifierMap[code];
    }

    // Handle punctuation and special characters
    const punctuationMap: Record<string, string> = {
      Semicolon: ";",
      Equal: "=",
      Comma: ",",
      Minus: "-",
      Period: ".",
      Slash: "/",
      Backquote: "`",
      BracketLeft: "[",
      Backslash: "\\",
      BracketRight: "]",
      Quote: "'",
    };

    if (punctuationMap[code]) {
      return punctuationMap[code];
    }

    // Parser names mirror KeyboardEvent.code without spaces.
    return code.toLowerCase();
  }

  // Fallback to e.key if e.code is not available
  if (e.key) {
    const key = e.key;

    // Handle special key names
    const keyMap: Record<string, string> = {
      Control: "ctrl",
      Alt: "alt",
      Shift: "shift",
      Meta: "win",
      OS: "win",
      CapsLock: "capslock",
      ArrowUp: "up",
      ArrowDown: "down",
      ArrowLeft: "left",
      ArrowRight: "right",
      Escape: "esc",
      " ": "space",
    };

    if (keyMap[key]) {
      return keyMap[key];
    }

    return key.toLowerCase();
  }

  // Last resort fallback
  return `unknown-${e.keyCode || e.which || 0}`;
};

/**
 * Capitalize a key name for display (e.g. "space" -> "Space", "f1" -> "F1")
 */
const capitalizeKey = (key: string): string => {
  // fn key: keep lowercase
  if (key === "fn") return "fn";
  // Function keys: f1 -> F1
  if (/^f\d+$/.test(key)) return key.toUpperCase();
  // Single char: a -> A
  if (key.length === 1) return key.toUpperCase();
  if (KEY_DISPLAY[key]) return KEY_DISPLAY[key];
  // Multi-word: capitalize first letter of each word
  return key.replace(/\b\w/g, (c) => c.toUpperCase());
};

/**
 * Format a single key part for display.
 * Handles _left/_right suffixes and capitalizes names.
 * e.g. "shift_left" -> "Left Shift", "alt" -> "Alt", "space" -> "Space"
 */
const formatKeyPart = (part: string): string => {
  const trimmed = part.trim();
  if (!trimmed) return "";

  if (trimmed.endsWith("_left")) {
    const name = trimmed.slice(0, -5);
    return `Left ${capitalizeKey(name)}`;
  }
  if (trimmed.endsWith("_right")) {
    const name = trimmed.slice(0, -6);
    return `Right ${capitalizeKey(name)}`;
  }

  return capitalizeKey(trimmed);
};

/**
 * Get display-friendly key combination string
 * Formats raw hotkey strings like "ctrl_left+shift+space" into
 * human-readable form like "Left Ctrl + Shift + Space"
 */
export const formatKeyCombination = (combination: string): string => {
  if (!combination) return "";
  return combination.split("+").map(formatKeyPart).join(" + ");
};

/**
 * Normalize modifier keys to handle left/right variants
 */
export const normalizeKey = (key: string): string => {
  // Handle left/right variants of modifier keys
  if (key.startsWith("left ") || key.startsWith("right ")) {
    const parts = key.split(" ");
    if (parts.length === 2) {
      // Return just the modifier name without left/right prefix
      return parts[1];
    }
  }
  return key;
};
