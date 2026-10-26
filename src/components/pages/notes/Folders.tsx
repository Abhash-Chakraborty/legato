import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { ChevronDown, Folder, FolderPlus } from "lucide-react";
import { commands, type Note, type NoteFolder } from "@/bindings";
import { ActionMenu } from "../../ui/ActionMenu";
import { Button } from "../../ui/Button";
import { Dialog } from "../../ui/Dialog";

const FOLDER_CHARS = 60;

/** Names a new folder, or renames one. Enter saves, Esc cancels. */
export const FolderDialog: React.FC<{
  open: boolean;
  initial?: string;
  title: string;
  action: string;
  onClose: () => void;
  onSave: (name: string) => Promise<boolean>;
}> = ({ open, initial = "", title, action, onClose, onSave }) => {
  const { t } = useTranslation();
  const [name, setName] = useState(initial);
  const [busy, setBusy] = useState(false);
  const ref = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) setName(initial);
  }, [open, initial]);

  const save = async () => {
    const clean = name.trim();
    if (!clean || busy) return;
    setBusy(true);
    const ok = await onSave(clean);
    setBusy(false);
    if (ok) onClose();
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => !next && onClose()}
      title={title}
      closeLabel={t("common.cancel")}
      initialFocusRef={ref}
      footer={
        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button onClick={save} disabled={busy || !name.trim()}>
            {action}
          </Button>
        </div>
      }
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          save();
        }}
      >
        <input
          ref={ref}
          value={name}
          maxLength={FOLDER_CHARS}
          onChange={(e) => setName(e.target.value)}
          placeholder={t("notes.folders.placeholder")}
          aria-label={t("notes.folders.name")}
          className="input w-full"
        />
      </form>
    </Dialog>
  );
};

/** Moves one note: to a folder, a new one, or out of its folder. */
export const MoveToFolder: React.FC<{
  note: Note;
  folders: NoteFolder[];
  className?: string;
  onClose: (restore: boolean) => void;
}> = ({ note, folders, className, onClose }) => {
  const { t } = useTranslation();
  const [naming, setNaming] = useState(false);
  const current = note.folder?.toLowerCase() ?? null;

  const move = async (folder: string | null) => {
    const result = await commands
      .moveNotesToFolder([note.id], folder)
      .catch((e) => ({ status: "error" as const, error: String(e) }));
    if (result.status === "error") {
      toast.error(t("notes.folders.moveFailed"), {
        description: result.error,
      });
      return false;
    }
    const was = note.folder ?? null;
    toast(
      folder
        ? t("notes.folders.moved", { folder })
        : t("notes.folders.removed", { folder: was ?? "" }),
      {
        action: {
          label: t("notes.toast.undo"),
          onClick: () => {
            commands.moveNotesToFolder([note.id], was).catch(() => {});
          },
        },
      },
    );
    return true;
  };

  return (
    <>
      {!naming && (
        <ActionMenu
          label={t("notes.folders.moveTo")}
          className={className}
          onClose={onClose}
          items={[
            ...folders.map((f) => ({
              id: `folder-${f.name}`,
              label: f.name,
              icon: Folder,
              checked: current === f.name.toLowerCase(),
              onSelect: () => {
                if (current !== f.name.toLowerCase()) move(f.name);
              },
            })),
            ...(folders.length ? ["separator" as const] : []),
            {
              id: "new",
              label: t("notes.folders.new"),
              icon: FolderPlus,
              onSelect: () => setNaming(true),
            },
            ...(note.folder
              ? [
                  {
                    id: "out",
                    label: t("notes.folders.takeOut", { folder: note.folder }),
                    onSelect: () => move(null),
                  },
                ]
              : []),
          ]}
        />
      )}
      <FolderDialog
        open={naming}
        title={t("notes.folders.newTitle")}
        action={t("notes.folders.createAndMove")}
        onClose={() => {
          setNaming(false);
          onClose(true);
        }}
        onSave={move}
      />
    </>
  );
};

/** Which folder the list shows, and the folders themselves: new, rename,
 *  delete (with Undo; the notes stay, in no folder). */
export const FolderFilter: React.FC<{
  folders: NoteFolder[];
  folder: string | null;
  onChange: (folder: string | null) => void;
}> = ({ folders, folder, onChange }) => {
  const { t } = useTranslation();
  const [menu, setMenu] = useState(false);
  const [naming, setNaming] = useState<"new" | "rename" | null>(null);
  const button = useRef<HTMLButtonElement>(null);

  const fail = (title: string) => (e: unknown) => {
    toast.error(title, { description: String(e) });
    return false;
  };

  const create = async (name: string) => {
    const result = await commands.addNoteFolder(name).catch(String);
    if (typeof result === "string" || result.status === "error")
      return fail(t("notes.folders.createFailed"))(
        typeof result === "string" ? result : result.error,
      );
    onChange(result.data);
    return true;
  };

  const rename = async (to: string) => {
    if (!folder) return false;
    const result = await commands.renameNoteFolder(folder, to).catch(String);
    if (typeof result === "string" || result.status === "error")
      return fail(t("notes.folders.renameFailed"))(
        typeof result === "string" ? result : result.error,
      );
    onChange(result.data);
    return true;
  };

  const remove = async () => {
    if (!folder) return;
    const name = folder;
    const result = await commands.deleteNoteFolder(name).catch(String);
    if (typeof result === "string" || result.status === "error") {
      fail(t("notes.folders.deleteFailed"))(
        typeof result === "string" ? result : result.error,
      );
      return;
    }
    onChange(null);
    const ids = result.data;
    toast(t("notes.folders.deleted", { folder: name }), {
      action: {
        label: t("notes.toast.undo"),
        onClick: async () => {
          await commands.addNoteFolder(name).catch(() => null);
          if (ids.length)
            await commands.moveNotesToFolder(ids, name).catch(() => null);
          onChange(name);
        },
      },
    });
  };

  const selected = folders.find(
    (f) => f.name.toLowerCase() === folder?.toLowerCase(),
  );

  return (
    <div className="relative">
      <button
        ref={button}
        type="button"
        onClick={() => setMenu(!menu)}
        aria-haspopup="menu"
        aria-expanded={menu}
        className={`inline-flex h-7 max-w-[150px] items-center gap-1.5 rounded-[var(--r-sm)] px-2 text-[13px] transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:duration-0 ${
          selected ? "font-medium text-teal-700" : "text-text-2"
        }`}
      >
        <Folder className="h-3.5 w-3.5 flex-none" aria-hidden="true" />
        <span className="truncate">
          {selected?.name ?? t("notes.folders.label")}
        </span>
        <ChevronDown className="h-3.5 w-3.5 flex-none" aria-hidden="true" />
      </button>
      {menu && (
        <ActionMenu
          label={t("notes.folders.label")}
          className="end-0 top-full z-10 mt-1 w-56"
          onClose={(restore) => {
            setMenu(false);
            if (restore) button.current?.focus();
          }}
          items={[
            {
              id: "all",
              label: t("notes.folders.all"),
              checked: !selected,
              onSelect: () => onChange(null),
            },
            ...folders.map((f) => ({
              id: `folder-${f.name}`,
              label: f.name,
              shortcut: String(f.notes),
              checked: selected?.name === f.name,
              onSelect: () => onChange(f.name),
            })),
            "separator" as const,
            {
              id: "new",
              label: t("notes.folders.new"),
              icon: FolderPlus,
              onSelect: () => setNaming("new"),
            },
            ...(selected
              ? [
                  {
                    id: "rename",
                    label: t("notes.folders.rename"),
                    onSelect: () => setNaming("rename"),
                  },
                  {
                    id: "delete",
                    label: t("notes.folders.delete"),
                    danger: true,
                    onSelect: remove,
                  },
                ]
              : []),
          ]}
        />
      )}
      <FolderDialog
        open={naming !== null}
        initial={naming === "rename" ? (selected?.name ?? "") : ""}
        title={
          naming === "rename"
            ? t("notes.folders.renameTitle")
            : t("notes.folders.newTitle")
        }
        action={
          naming === "rename"
            ? t("notes.folders.save")
            : t("notes.folders.create")
        }
        onClose={() => setNaming(null)}
        onSave={naming === "rename" ? rename : create}
      />
    </div>
  );
};
