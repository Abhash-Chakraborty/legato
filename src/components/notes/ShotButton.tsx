import React, { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Camera, Check } from "@/icons";
import { commands } from "@/bindings";
import { Button } from "../ui/Button";

/** Takes a screenshot of the screen the pointer is on, into the note being
 *  recorded. The screen is grabbed at the click; the tick shows it was. */
export const ShotButton: React.FC<{ disabled?: boolean }> = ({ disabled }) => {
  const { t } = useTranslation();
  const [taken, setTaken] = useState(false);

  const take = async () => {
    const result = await commands
      .takeNoteScreenshot()
      .catch(() => ({ status: "error" as const, error: "failed" }));
    if (result.status === "error")
      toast.error(
        result.error === "idle"
          ? t("notes.shots.idle")
          : t("notes.shots.failed"),
      );
  };

  // Taken here, on the bar or with the shortcut: the tick shows it.
  useEffect(() => {
    let timer = 0;
    const unlisten = listen("note-shot-taken", () => {
      setTaken(true);
      window.clearTimeout(timer);
      timer = window.setTimeout(() => setTaken(false), 1400);
    });
    return () => {
      window.clearTimeout(timer);
      unlisten.then((off) => off());
    };
  }, []);

  return (
    <Button
      variant="secondary"
      size="sm"
      disabled={disabled}
      onClick={take}
      aria-label={t("notes.shots.take")}
      title={taken ? t("notes.shots.taken") : t("notes.shots.takeHelp")}
    >
      {taken ? (
        <Check className="h-4 w-4 text-teal-700" aria-hidden="true" />
      ) : (
        <Camera className="h-4 w-4" aria-hidden="true" />
      )}
    </Button>
  );
};
