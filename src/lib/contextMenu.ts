import { Menu, type MenuOptions } from "@tauri-apps/api/menu";
import { LogicalPosition } from "@tauri-apps/api/dpi";
import { commands } from "@/bindings";
import i18n from "@/i18n";

type Field = HTMLInputElement | HTMLTextAreaElement;
type Item = NonNullable<MenuOptions["items"]>[number];

// Input types that hold text the person can select; the rest (checkbox,
// range, number…) have no selection to cut or copy.
const TEXT_TYPES = new Set(["text", "search", "url", "tel", "email", ""]);

const textField = (el: Element | null): Field | null => {
  if (el instanceof HTMLTextAreaElement) return el;
  if (el instanceof HTMLInputElement && TEXT_TYPES.has(el.type)) return el;
  return null;
};

const editableRoot = (el: Element | null): HTMLElement | null => {
  const host = el?.closest<HTMLElement>("[contenteditable]");
  return host?.isContentEditable ? host : null;
};

/** What the right-click landed on, and what it can do there. */
const menuItems = (target: Element | null): Item[] => {
  const t = i18n.t.bind(i18n);
  const field = textField(target);
  const root = field ? null : editableRoot(target);

  if (field) {
    const start = field.selectionStart ?? 0;
    const end = field.selectionEnd ?? 0;
    const selected = field.value.slice(start, end);
    const editable = !field.readOnly && !field.disabled;
    const restore = () => {
      field.focus();
      field.setSelectionRange(start, end);
    };
    const items: Item[] = [];
    if (editable)
      items.push({
        id: "cut",
        text: t("contextMenu.cut"),
        enabled: !!selected,
        action: () => {
          navigator.clipboard
            .writeText(selected)
            .then(() => {
              restore();
              document.execCommand("delete");
            })
            .catch(() => {});
        },
      });
    items.push({
      id: "copy",
      text: t("contextMenu.copy"),
      enabled: !!selected,
      action: () => {
        navigator.clipboard.writeText(selected).catch(() => {});
      },
    });
    if (editable)
      items.push({
        id: "paste",
        text: t("contextMenu.paste"),
        action: () => {
          commands
            .readClipboardText()
            .then((text) => {
              if (!text) return;
              restore();
              // insertText keeps the field's own undo history and fires
              // React's onChange, which setting `value` would not.
              document.execCommand("insertText", false, text);
            })
            .catch(() => {});
        },
      });
    items.push(
      { item: "Separator" },
      {
        id: "select-all",
        text: t("contextMenu.selectAll"),
        enabled: !!field.value,
        action: () => {
          field.focus();
          field.select();
        },
      },
    );
    return items;
  }

  const selection = window.getSelection();
  const selected = selection?.toString() ?? "";
  if (root) {
    const range =
      selection && selection.rangeCount ? selection.getRangeAt(0) : null;
    const restore = () => {
      root.focus();
      if (range && selection) {
        selection.removeAllRanges();
        selection.addRange(range);
      }
    };
    return [
      {
        id: "cut",
        text: t("contextMenu.cut"),
        enabled: !!selected,
        action: () => {
          navigator.clipboard
            .writeText(selected)
            .then(() => {
              restore();
              document.execCommand("delete");
            })
            .catch(() => {});
        },
      },
      {
        id: "copy",
        text: t("contextMenu.copy"),
        enabled: !!selected,
        action: () => {
          navigator.clipboard.writeText(selected).catch(() => {});
        },
      },
      {
        id: "paste",
        text: t("contextMenu.paste"),
        action: () => {
          commands
            .readClipboardText()
            .then((text) => {
              if (!text) return;
              restore();
              document.execCommand("insertText", false, text);
            })
            .catch(() => {});
        },
      },
      { item: "Separator" },
      {
        id: "select-all",
        text: t("contextMenu.selectAll"),
        action: () => {
          root.focus();
          selection?.selectAllChildren(root);
        },
      },
    ];
  }

  // Plain text the person selected, such as a transcript or a dictation.
  if (selected.trim())
    return [
      {
        id: "copy",
        text: t("contextMenu.copy"),
        action: () => {
          navigator.clipboard.writeText(selected).catch(() => {});
        },
      },
    ];
  return [];
};

/** Replaces the webview's own right-click menu (Back, Refresh, Print, Save
 *  as…) in a window. Components with their own menu (history rows, transcript
 *  lines, the bubble) call `preventDefault` first and are left alone; text
 *  gets Cut, Copy, Paste and Select all; everything else gets no menu. In
 *  development, Shift+right-click keeps the browser menu for Inspect. */
export const installContextMenu = () => {
  window.addEventListener("contextmenu", (e) => {
    if (e.defaultPrevented) return;
    if (import.meta.env.DEV && e.shiftKey) return;
    e.preventDefault();
    const target = e.target instanceof Element ? e.target : null;
    const items = menuItems(target);
    if (!items.length) return;
    const at = new LogicalPosition(e.clientX, e.clientY);
    Menu.new({ items })
      .then((menu) => menu.popup(at))
      .catch(() => {});
  });
};
