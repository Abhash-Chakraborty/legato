import React, { useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Trash2 } from "lucide-react";
import {
  commands,
  type AiStatus,
  type PostProcessProvider,
  type Transform,
  type TransformOutput,
} from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { Button } from "../../ui/Button";
import { Dropdown } from "../../ui/Dropdown";
import { Sheet } from "../../ui/Sheet";
import { Requires, SettingContainer } from "../../ui/SettingContainer";
import { ShortcutInput } from "../../settings/ShortcutInput";
import { TryIt } from "./TryIt";

const OUTPUTS: TransformOutput[] = ["replace", "below", "copy"];
const VARIABLES = ["{{app}}", "{{dictionary}}", "{{clipboard}}"] as const;

interface TransformEditorProps {
  /** The transform to edit, or a new one with an empty id. */
  transform: Transform;
  providers: PostProcessProvider[];
  status: AiStatus | null;
  onClose: () => void;
  onSaved: (transform: Transform, created: boolean) => void;
  onDelete: (transform: Transform) => void;
}

/** Everything about one transform: what it's called, what it asks the model,
 *  which model, where the result goes, its shortcut, and a test run on the
 *  draft before saving. */
export const TransformEditor: React.FC<TransformEditorProps> = ({
  transform,
  providers,
  status,
  onClose,
  onSaved,
  onDelete,
}) => {
  const { t } = useTranslation();
  const { settings, refreshSettings } = useSettings();
  const nameId = useId();
  const descriptionId = useId();
  const promptId = useId();
  const modelId = useId();
  const nameRef = useRef<HTMLInputElement>(null);
  const promptRef = useRef<HTMLTextAreaElement>(null);
  const created = transform.id === "";
  const [draft, setDraft] = useState<Transform>(transform);
  const [ownModel, setOwnModel] = useState(
    !!(transform.provider_id && transform.model),
  );
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const set = (patch: Partial<Transform>) => {
    setDraft({ ...draft, ...patch });
    setError(null);
  };

  const providerId = draft.provider_id ?? providers[0]?.id ?? "";
  const toSave: Transform = {
    ...draft,
    name: draft.name.trim(),
    description: draft.description?.trim() ?? "",
    provider_id: ownModel ? providerId : null,
    model: ownModel ? draft.model?.trim() || null : null,
  };

  const insertVariable = (variable: string) => {
    const el = promptRef.current;
    const at = el ? el.selectionStart : draft.prompt.length;
    const end = el ? el.selectionEnd : at;
    const prompt =
      draft.prompt.slice(0, at) + variable + draft.prompt.slice(end);
    set({ prompt });
    requestAnimationFrame(() => {
      el?.focus();
      el?.setSelectionRange(at + variable.length, at + variable.length);
    });
  };

  const save = async () => {
    if (saving) return;
    if (!toSave.name) {
      setError(t("transforms.editor.errors.name"));
      nameRef.current?.focus();
      return;
    }
    if (!toSave.prompt.trim()) {
      setError(t("transforms.editor.errors.prompt"));
      promptRef.current?.focus();
      return;
    }
    if (ownModel && !toSave.model) {
      setError(t("transforms.editor.errors.model"));
      return;
    }
    setSaving(true);
    const result = await commands.saveTransform(toSave);
    setSaving(false);
    if (result.status === "error") {
      setError(result.error);
      return;
    }
    await refreshSettings();
    onSaved(result.data, created);
  };

  const bindingId = `transform:${transform.id}`;
  const binding = settings?.bindings?.[bindingId]?.current_binding ?? "";

  return (
    <Sheet
      open
      wide
      title={
        created
          ? t("transforms.editor.newTitle")
          : t("transforms.editor.editTitle")
      }
      description={
        transform.built_in
          ? t("transforms.editor.builtInDescription")
          : t("transforms.editor.description")
      }
      onClose={onClose}
      onSubmit={save}
      closeLabel={t("transforms.editor.cancel")}
      initialFocusRef={nameRef}
      footer={
        <>
          {!created && !transform.built_in && (
            <Button
              type="button"
              variant="danger-ghost"
              size="md"
              onClick={() => onDelete(transform)}
              className="me-auto"
            >
              <Trash2 className="h-4 w-4" aria-hidden="true" />
              {t("transforms.editor.delete")}
            </Button>
          )}
          <Button
            type="button"
            variant="ghost"
            onClick={onClose}
            className={!created && !transform.built_in ? "" : "ms-auto"}
          >
            {t("transforms.editor.cancel")}
          </Button>
          <Button type="submit" variant="primary" disabled={saving}>
            {t("transforms.editor.save")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-5">
        <div className="grid gap-4 min-[520px]:grid-cols-[1fr_1.4fr]">
          <div>
            <label
              htmlFor={nameId}
              className="mb-1.5 block text-[13px] font-medium"
            >
              {t("transforms.editor.name")}
            </label>
            <input
              id={nameId}
              ref={nameRef}
              className="input w-full"
              value={draft.name}
              maxLength={40}
              onChange={(e) => set({ name: e.target.value })}
              placeholder={t("transforms.editor.namePlaceholder")}
              autoComplete="off"
            />
          </div>
          <div>
            <label
              htmlFor={descriptionId}
              className="mb-1.5 block text-[13px] font-medium"
            >
              {t("transforms.editor.summary")}
            </label>
            <input
              id={descriptionId}
              className="input w-full"
              value={draft.description ?? ""}
              maxLength={90}
              onChange={(e) => set({ description: e.target.value })}
              placeholder={t("transforms.editor.summaryPlaceholder")}
              autoComplete="off"
            />
          </div>
        </div>

        <div>
          <label
            htmlFor={promptId}
            className="mb-1.5 block text-[13px] font-medium"
          >
            {t("transforms.editor.prompt")}
          </label>
          <textarea
            id={promptId}
            ref={promptRef}
            value={draft.prompt}
            onChange={(e) => set({ prompt: e.target.value })}
            placeholder={t("transforms.editor.promptPlaceholder")}
            className="input min-h-[150px] w-full resize-none py-2.5 leading-relaxed"
            spellCheck={false}
          />
          <div className="mt-1.5 flex flex-wrap items-center gap-1.5 text-[13px] text-text-3">
            <span>{t("transforms.editor.variables")}</span>
            {VARIABLES.map((v) => (
              <button
                key={v}
                type="button"
                onClick={() => insertVariable(v)}
                title={t(`transforms.editor.variable.${v.slice(2, -2)}`)}
                className="rounded-[var(--r-xs)] bg-surface-2 px-1.5 py-0.5 font-mono text-xs text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text hover:duration-0"
              >
                {v}
              </button>
            ))}
          </div>
        </div>

        <fieldset>
          <legend className="mb-1.5 text-[13px] font-medium">
            {t("transforms.editor.model")}
          </legend>
          <div className="flex flex-col gap-2 text-sm">
            <label className="flex cursor-pointer items-center gap-2.5">
              <input
                type="radio"
                name="model"
                className="h-4 w-4 accent-[var(--teal-600)]"
                checked={!ownModel}
                onChange={() => setOwnModel(false)}
              />
              {t("transforms.editor.modelDefault")}
            </label>
            <label className="flex cursor-pointer items-center gap-2.5">
              <input
                type="radio"
                name="model"
                className="h-4 w-4 accent-[var(--teal-600)]"
                checked={ownModel}
                onChange={() => setOwnModel(true)}
              />
              {t("transforms.editor.modelOwn")}
            </label>
          </div>
          {ownModel && (
            <div className="mt-2.5 grid gap-2 ps-[26px] min-[520px]:grid-cols-[180px_1fr]">
              <Dropdown
                className="w-full"
                ariaLabel={t("transforms.editor.provider")}
                options={providers.map((p) => ({
                  value: p.id,
                  label: p.id === "custom" ? t("ai.yourServer") : p.label,
                }))}
                selectedValue={providerId}
                onSelect={(value) => set({ provider_id: value })}
              />
              <input
                id={modelId}
                aria-label={t("transforms.editor.modelName")}
                className="input w-full"
                value={draft.model ?? ""}
                onChange={(e) => set({ model: e.target.value })}
                placeholder={t("transforms.editor.modelPlaceholder")}
                autoComplete="off"
                spellCheck={false}
              />
              <p className="text-[13px] text-text-3 min-[520px]:col-span-2">
                {t("transforms.editor.modelHelp")}
              </p>
            </div>
          )}
        </fieldset>

        <fieldset>
          <legend className="mb-1.5 text-[13px] font-medium">
            {t("transforms.editor.output")}
          </legend>
          <div className="flex flex-col gap-2 text-sm">
            {OUTPUTS.map((output) => (
              <label
                key={output}
                className="flex cursor-pointer items-start gap-2.5"
              >
                <input
                  type="radio"
                  name="output"
                  className="mt-0.5 h-4 w-4 accent-[var(--teal-600)]"
                  checked={(draft.output ?? "replace") === output}
                  onChange={() => set({ output })}
                />
                <span>
                  {t(`transforms.editor.outputs.${output}`)}
                  <span className="block text-[13px] text-text-3">
                    {t(`transforms.editor.outputs.${output}Help`)}
                  </span>
                </span>
              </label>
            ))}
          </div>
        </fieldset>

        <label className="flex cursor-pointer items-start gap-2.5 text-sm">
          <input
            type="checkbox"
            className="check mt-px"
            checked={draft.show_changes !== false}
            onChange={(e) => set({ show_changes: e.target.checked })}
          />
          <span>
            {t("transforms.editor.showChanges")}
            <span className="block text-[13px] text-text-3">
              {t("transforms.editor.showChangesHelp")}
            </span>
          </span>
        </label>

        <div className="rounded-[var(--r-md)] border border-line px-4">
          {created ? (
            <Requires
              met={false}
              reason={t("transforms.editor.shortcutAfterSave")}
            >
              <SettingContainer
                title={t("transforms.editor.shortcut")}
                description={t("transforms.editor.shortcutHelp")}
                grouped
              >
                <span className="text-sm text-text-3">
                  {t("settings.general.shortcut.notSet")}
                </span>
              </SettingContainer>
            </Requires>
          ) : (
            <ShortcutInput
              shortcutId={bindingId}
              title={t("transforms.editor.shortcut")}
              description={t("transforms.editor.shortcutHelp")}
              grouped
            />
          )}
          {!created && binding && (
            <div className="-mt-1 pb-3 text-end">
              <button
                type="button"
                className="link text-[13px]"
                onClick={async () => {
                  await commands.clearTransformShortcut(transform.id);
                  await refreshSettings();
                }}
              >
                {t("transforms.editor.removeShortcut")}
              </button>
            </div>
          )}
        </div>

        {error && (
          <p role="alert" className="text-[13px] text-danger">
            {error}
          </p>
        )}

        <div>
          <h3 className="mb-2 text-[13px] font-medium">
            {t("transforms.editor.test")}
          </h3>
          <TryIt
            transforms={[toSave]}
            providers={providers}
            status={status}
            compact
          />
        </div>
      </div>
    </Sheet>
  );
};
