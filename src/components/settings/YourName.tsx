import React, { useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { X } from "lucide-react";
import { commands, type YourName as Saved } from "@/bindings";
import { Input } from "../ui/Input";
import { SettingContainer } from "../ui/SettingContainer";

/** What people call you, set once: notes give tasks meant for you to You,
 *  and the Dictionary keeps it as a person's name with the ways the model
 *  mishears it. */
export const YourName: React.FC<{ grouped?: boolean }> = ({
  grouped = false,
}) => {
  const { t } = useTranslation();
  const [saved, setSaved] = useState<Saved | null>(null);
  const [name, setName] = useState("");
  const [heard, setHeard] = useState<string[]>([]);
  const [heardInput, setHeardInput] = useState("");
  const heardRef = useRef<HTMLInputElement>(null);
  const heardId = useId();

  useEffect(() => {
    commands
      .getYourName()
      .then((result) => {
        if (result.status !== "ok") return;
        setSaved(result.data);
        setName(result.data.name);
        setHeard(result.data.heard);
      })
      .catch(() => {});
  }, []);

  const save = async (nextName: string, nextHeard: string[]) => {
    const clean = nextName.trim();
    if (
      saved &&
      clean === saved.name &&
      nextHeard.join("\n") === saved.heard.join("\n")
    )
      return;
    // A new name starts from the forms its Dictionary word already has;
    // the ones shown were heard for the old name.
    const renamed =
      clean.toLowerCase() !== (saved?.name ?? "").trim().toLowerCase();
    const result = await commands
      .setYourName(clean, clean && !renamed ? nextHeard : [])
      .catch(() => null);
    if (result?.status !== "ok") {
      toast.error(t("settings.yourName.failed"), {
        description: result?.status === "error" ? result.error : undefined,
      });
      return;
    }
    setSaved(result.data);
    setName(result.data.name);
    setHeard(result.data.heard);
  };

  const addChip = () => {
    const form = heardInput.trim().replace(/,$/, "").trim();
    setHeardInput("");
    if (!form || heard.some((h) => h.toLowerCase() === form.toLowerCase()))
      return;
    const next = [...heard, form];
    setHeard(next);
    save(name, next);
  };

  return (
    <SettingContainer
      title={t("settings.yourName.title")}
      description={t("settings.yourName.description")}
      grouped={grouped}
      layout="stacked"
    >
      <div className="grid gap-3 sm:grid-cols-[minmax(0,220px)_minmax(0,1fr)]">
        <label className="block text-[13px] font-medium">
          {t("settings.yourName.name")}
          <Input
            value={name}
            maxLength={100}
            placeholder={t("settings.yourName.placeholder")}
            onChange={(e) => setName(e.target.value)}
            onBlur={() => save(name, heard)}
            onKeyDown={(e) => {
              if (e.key === "Enter") e.currentTarget.blur();
            }}
            autoComplete="off"
            spellCheck={false}
            className="mt-1 w-full"
          />
        </label>
        <div>
          <label htmlFor={heardId} className="block text-[13px] font-medium">
            {t("settings.yourName.heard")}
          </label>
          <div
            className={`mt-1 flex min-h-[34px] cursor-text flex-wrap items-center gap-1.5 rounded-[var(--r-md)] border border-line-strong bg-surface px-2 py-[3px] transition-[border-color,box-shadow] duration-[var(--dur-1)] focus-within:border-teal-600 focus-within:shadow-[0_0_0_3px_var(--teal-50)] ${name.trim() ? "" : "pointer-events-none opacity-50"}`}
            onClick={() => heardRef.current?.focus()}
          >
            {heard.map((h, i) => (
              <span
                key={h}
                className="inline-flex h-6 items-center gap-1 rounded-[var(--r-xs)] bg-surface-3 ps-2 pe-1 text-[13px]"
              >
                {h}
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    const next = heard.filter((_, j) => j !== i);
                    setHeard(next);
                    save(name, next);
                  }}
                  aria-label={t("dictionary.editor.removeHeard", { heard: h })}
                  title={t("dictionary.editor.removeHeard", { heard: h })}
                  className="grid h-[18px] w-[18px] place-items-center rounded text-text-3 hover:bg-surface-4 hover:text-text"
                >
                  <X className="h-3 w-3" aria-hidden="true" />
                </button>
              </span>
            ))}
            <input
              id={heardId}
              ref={heardRef}
              value={heardInput}
              disabled={!name.trim()}
              onChange={(e) => setHeardInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === ",") {
                  e.preventDefault();
                  addChip();
                } else if (
                  e.key === "Backspace" &&
                  !heardInput &&
                  heard.length > 0
                ) {
                  const next = heard.slice(0, -1);
                  setHeard(next);
                  save(name, next);
                }
              }}
              onBlur={addChip}
              placeholder={t("settings.yourName.heardPlaceholder")}
              className="h-[26px] min-w-[120px] flex-1 border-0 bg-transparent text-sm text-text outline-none placeholder:text-text-4"
              autoComplete="off"
              spellCheck={false}
            />
          </div>
        </div>
      </div>
    </SettingContainer>
  );
};
