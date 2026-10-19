import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Info } from "lucide-react";
import {
  commands,
  type AudioApp,
  type AudioDevice,
  type NoteKind,
} from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { Button } from "../../ui/Button";
import { Dialog } from "../../ui/Dialog";
import { Dropdown } from "../../ui/Dropdown";
import { Segmented } from "../../ui/Segmented";

/** "Everything" in the system audio picker. */
const EVERYTHING = "everything";

interface RecordDialogProps {
  open: boolean;
  kind: NoteKind;
  onClose: () => void;
  /** The new note, once it is recording. */
  onStarted: (noteId: number) => void;
}

/** Record: a meeting or a voice memo, its title, the microphone and, for a
 *  meeting, whose sound is the other side. */
export const RecordDialog: React.FC<RecordDialogProps> = ({
  open,
  kind: initialKind,
  onClose,
  onStarted,
}) => {
  const { t, i18n } = useTranslation();
  const { settings, updateSetting, refreshSettings } = useSettings();
  const [kind, setKind] = useState<NoteKind>(initialKind);
  const [title, setTitle] = useState("");
  const [mics, setMics] = useState<AudioDevice[]>([]);
  const [mic, setMic] = useState<string | null>(null);
  const [apps, setApps] = useState<AudioApp[]>([]);
  const [output, setOutput] = useState<string | null>(null);
  const [source, setSource] = useState(EVERYTHING);
  const [starting, setStarting] = useState(false);
  const titleRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!open) return;
    setKind(initialKind);
    setTitle("");
    setStarting(false);
    commands
      .getAvailableMicrophones()
      .then((r) => {
        if (r.status !== "ok") return;
        setMics(r.data);
        const chosen = settings?.selected_microphone;
        setMic(
          r.data.find((d) => d.name === chosen)?.name ??
            r.data.find((d) => d.is_default)?.name ??
            r.data[0]?.name ??
            null,
        );
      })
      .catch(() => {});
    commands
      .getNotesAudioSources()
      .then(([found, out]) => {
        setApps(found);
        setOutput(out);
        const call = found.find((a) => a.call);
        setSource(call ? String(call.pid) : EVERYTHING);
      })
      .catch(() => {});
  }, [open, initialKind]);

  const when = new Date().toLocaleString(i18n.language, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
  const placeholder = t(`notes.record.defaultTitle.${kind}`, { when });

  const start = async () => {
    if (starting) return;
    setStarting(true);
    const result = await commands.startNotes({
      kind,
      title: title.trim() || null,
      microphone: mic,
      app_pid:
        kind === "meeting" && source !== EVERYTHING ? Number(source) : null,
    });
    setStarting(false);
    if (result.status === "error") {
      toast.error(t("notes.toast.startFailed"), { description: result.error });
      return;
    }
    onStarted(result.data.id);
    onClose();
  };

  const live = settings?.notes_live_transcript ?? true;
  const reminder = settings?.notes_reminder ?? true;
  const everything = output
    ? t("notes.record.everythingOn", { device: output })
    : t("notes.record.everything");

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => !next && onClose()}
      title={t("notes.record.title")}
      closeLabel={t("notes.record.cancel")}
      initialFocusRef={titleRef}
      footer={
        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={onClose}>
            {t("notes.record.cancel")}
          </Button>
          <Button onClick={start} disabled={starting || !mics.length}>
            <span
              aria-hidden="true"
              className="h-2 w-2 rounded-full bg-danger"
            />
            {t("notes.record.start")}
          </Button>
        </div>
      }
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          start();
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter" && e.ctrlKey) {
            e.preventDefault();
            start();
          }
        }}
        className="grid gap-4"
      >
        <div>
          <Segmented
            className="w-full"
            label={t("notes.record.kind")}
            value={kind}
            onChange={setKind}
            options={[
              { value: "meeting", label: t("notes.kind.meeting") },
              { value: "memo", label: t("notes.kind.memo") },
            ]}
          />
          <p className="mt-2 text-[13px] text-text-3">
            {t(`notes.record.kindHelp.${kind}`)}
          </p>
        </div>
        <label className="grid gap-1.5">
          <span className="text-sm font-medium">
            {t("notes.record.titleLabel")}
          </span>
          <input
            ref={titleRef}
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder={placeholder}
            className="input w-full"
          />
        </label>
        <div className="grid gap-1.5">
          <span id="notes-mic" className="text-sm font-medium">
            {t("notes.record.microphone")}
          </span>
          {mics.length ? (
            <Dropdown
              ariaLabel={t("notes.record.microphone")}
              selectedValue={mic}
              onSelect={setMic}
              options={mics.map((d) => ({ value: d.name, label: d.name }))}
            />
          ) : (
            <p className="text-[13px] text-danger">{t("notes.record.noMic")}</p>
          )}
        </div>
        {kind === "meeting" && (
          <div className="grid gap-1.5">
            <span className="text-sm font-medium">
              {t("notes.record.system")}
            </span>
            <Dropdown
              ariaLabel={t("notes.record.system")}
              selectedValue={source}
              onSelect={setSource}
              options={[
                { value: EVERYTHING, label: everything },
                ...apps.map((a) => ({
                  value: String(a.pid),
                  label: a.active
                    ? t("notes.record.appPlaying", { app: a.name })
                    : a.name,
                })),
              ]}
            />
            <p className="text-[13px] text-text-3">
              {source === EVERYTHING
                ? t("notes.record.systemHelpEverything")
                : t("notes.record.systemHelpApp")}
            </p>
          </div>
        )}
        <label className="flex items-center gap-2.5 text-sm">
          <input
            type="checkbox"
            className="check"
            checked={live}
            onChange={(e) =>
              updateSetting("notes_live_transcript", e.target.checked)
            }
          />
          {t("notes.record.live")}
        </label>
        {reminder && (
          <div className="flex items-center gap-2.5 rounded-[var(--r-md)] bg-surface-2 px-3 py-2.5 text-[13px] text-text-2">
            <Info
              className="h-4 w-4 flex-none text-text-3"
              aria-hidden="true"
            />
            <span className="flex-1">{t("notes.record.reminder")}</span>
            <button
              type="button"
              onClick={async () => {
                await commands.dismissNotesReminder();
                refreshSettings();
              }}
              className="font-medium text-teal-700 underline-offset-2 hover:underline"
            >
              {t("notes.record.reminderHide")}
            </button>
          </div>
        )}
      </form>
    </Dialog>
  );
};
