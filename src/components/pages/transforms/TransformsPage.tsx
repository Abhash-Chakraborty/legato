import React, { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Mic, Plus } from "lucide-react";
import { toast } from "sonner";
import { commands, type Transform } from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { useSettingsStore } from "../../../stores/settingsStore";
import { formatKeyCombination } from "../../../lib/utils/keyboard";
import { useAiStore, watchAi } from "../../../stores/aiStore";
import {
  useTransformsStore,
  watchTransformRuns,
} from "../../../stores/transformsStore";
import { AiStatusLine } from "../../shared/AiStatusLine";
import { Button } from "../../ui/Button";
import { Dropdown } from "../../ui/Dropdown";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { Requires, SettingContainer } from "../../ui/SettingContainer";
import { localDay } from "../home/format";
import { keyCaps, usageOf } from "./list";
import { modelLine } from "./model";
import { RecentRuns } from "./RecentRuns";
import { KeyCaps, TransformCard } from "./TransformCard";
import { TransformEditor } from "./TransformEditor";
import { TryIt } from "./TryIt";
import { reorder, useGridDrag } from "./useGridDrag";

const NONE = "__none__";

/** A switch for one setting, inside a settings row. */
const RowSwitch: React.FC<{
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}> = ({ label, checked, onChange }) => (
  <button
    type="button"
    role="switch"
    aria-checked={checked}
    aria-label={label}
    className="toggle"
    onClick={() => onChange(!checked)}
  />
);

/** Rewrites of dictated or selected text with your AI model: the transforms,
 *  Ask, what runs after every dictation, a place to try them, and what they
 *  did lately. */
export const TransformsPage: React.FC = () => {
  const { t, i18n } = useTranslation();
  const { settings, getSetting, updateSetting, refreshSettings } =
    useSettings();
  const status = useAiStore((s) => s.status);
  const runs = useTransformsStore((s) => s.runs);
  const [editing, setEditing] = useState<Transform | null>(null);

  useEffect(watchAi, []);
  useEffect(() => watchTransformRuns(), []);

  const enabled = getSetting("post_process_enabled") ?? false;
  const transforms = useMemo(() => settings?.transforms ?? [], [settings]);
  const providers = settings?.post_process_providers ?? [];
  const usable = transforms.filter((x) => x.enabled !== false);
  const autoId = getSetting("auto_transform_id") ?? null;
  const dictateId = getSetting("dictate_transform_id") ?? "polish";
  const minWords = getSetting("auto_transform_min_words") ?? 5;

  const keysOf = (id: string) =>
    keyCaps(
      formatKeyCombination(settings?.bindings?.[id]?.current_binding ?? ""),
    );

  const formatWhen = (at: number) => {
    const now = Date.now() / 1000;
    const day = localDay(at);
    if (day === localDay(now))
      return new Date(at * 1000).toLocaleTimeString(i18n.language, {
        hour: "numeric",
        minute: "2-digit",
      });
    if (day === localDay(now - 86400)) return t("transforms.card.yesterday");
    return new Date(at * 1000).toLocaleDateString(i18n.language, {
      month: "short",
      day: "numeric",
    });
  };

  const openNew = useCallback(async () => {
    const draft = await commands.newTransform().catch(() => null);
    if (draft) setEditing(draft);
  }, []);

  // N starts a new transform from anywhere on the page.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = (e.target as HTMLElement).closest(
        "input, textarea, [contenteditable=true], [role=dialog], [role=menu], [role=listbox]",
      );
      if (typing || editing || e.ctrlKey || e.altKey || e.metaKey) return;
      if (e.key.toLowerCase() === "n") {
        e.preventDefault();
        openNew();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [editing, openNew]);

  const report = async (
    result: { status: "ok" } | { status: "error"; error: string },
  ) => {
    if (result.status === "error") toast.error(result.error);
    await refreshSettings();
  };

  // Dragging a card: the new order shows at once, the backend confirms it.
  const ids = transforms.map((x) => x.id);
  const grid = useGridDrag(ids, "[data-transform]", (id, to) => {
    const from = ids.indexOf(id);
    useSettingsStore.setState((s) => ({
      settings: s.settings
        ? { ...s.settings, transforms: reorder(transforms, from, to) }
        : null,
    }));
    commands.moveTransform(id, to).then(report);
  });

  const remove = async (transform: Transform) => {
    const result = await commands.deleteTransform(transform.id);
    if (result.status === "error") {
      toast.error(result.error);
      return;
    }
    await refreshSettings();
    const deleted = result.data;
    toast(t("transforms.toast.deleted", { name: transform.name }), {
      action: {
        label: t("transforms.toast.undo"),
        onClick: async () => {
          await commands.restoreTransform(deleted).catch(() => {});
          await refreshSettings();
        },
      },
    });
  };

  const selectionKeys = keysOf("transform_selection");
  const askKeys = keysOf("ask");
  const dictateKeys = formatKeyCombination(
    settings?.bindings?.transcribe_with_post_process?.current_binding ?? "",
  );
  const askOn = getSetting("ask_enabled") ?? true;
  const options = usable.map((x) => ({ value: x.id, label: x.name }));

  return (
    <div className="max-w-[980px]">
      <div className="mb-6 flex min-h-10 flex-wrap items-center justify-between gap-4">
        <h1 className="text-[26px] font-semibold leading-tight tracking-[-0.015em]">
          {t("transforms.title")}
        </h1>
        <div className="flex items-center gap-3">
          {selectionKeys.length > 0 && (
            <span
              className="hidden items-center gap-1 rounded-[var(--r-sm)] bg-surface-2 py-1 ps-1.5 pe-2.5 text-[13px] text-text-2 min-[900px]:inline-flex"
              title={t("transforms.selectionHelp")}
            >
              <KeyCaps keys={selectionKeys} />
              <span className="ms-1">{t("transforms.selectionChip")}</span>
            </span>
          )}
          <span className="flex items-center gap-2 text-sm text-text-2">
            {enabled ? t("transforms.on") : t("transforms.offShort")}
            <RowSwitch
              label={t("transforms.switch")}
              checked={enabled}
              onChange={(on) => updateSetting("post_process_enabled", on)}
            />
          </span>
          <Button onClick={openNew}>
            <Plus className="h-4 w-4" aria-hidden="true" />
            {t("transforms.new")}
          </Button>
        </div>
      </div>
      <p className="-mt-3.5 max-w-[640px] text-sm text-text-2">
        {t("transforms.subtitle")}
      </p>
      <AiStatusLine className="mt-2" />

      {!enabled && (
        <div className="mt-5 flex flex-wrap items-center justify-between gap-3 rounded-[var(--r-md)] bg-surface-2 px-4 py-3">
          <p className="text-sm text-text-2">{t("transforms.off")}</p>
          <Button
            variant="secondary"
            size="sm"
            onClick={() => updateSetting("post_process_enabled", true)}
          >
            {t("transforms.turnOn")}
          </Button>
        </div>
      )}

      <div className="mt-6 mb-8 flex gap-4 rounded-[var(--r-lg)] border border-line bg-surface p-4">
        <span className="grid h-9 w-9 flex-none place-items-center rounded-[var(--r-md)] bg-surface-2 text-text-2">
          <Mic className="h-4 w-4" aria-hidden="true" />
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <h2 className="text-[15px] font-medium">
              {t("transforms.ask.title")}
            </h2>
            {askKeys.length > 0 && <KeyCaps keys={askKeys} />}
          </div>
          <p className="mt-1 text-sm text-text-2">{t("transforms.ask.body")}</p>
          <p className="mt-1.5 text-[13px] text-text-3">
            {t("transforms.ask.examples")}
          </p>
        </div>
        <div className="flex flex-none items-start gap-3">
          <span className="hidden pt-1 text-xs text-text-3 min-[760px]:inline">
            {status && status.kind !== "none"
              ? modelLine(
                  { id: "", name: "", prompt: "" },
                  providers,
                  status,
                  t,
                )
              : null}
          </span>
          <RowSwitch
            label={t("transforms.ask.switch")}
            checked={askOn}
            onChange={(on) => updateSetting("ask_enabled", on)}
          />
        </div>
      </div>

      <section className="mb-8">
        <h2 className="text-[17px] font-semibold tracking-[-0.01em]">
          {t("transforms.yours.title")}
        </h2>
        <p className="mb-3 mt-1 text-[13px] text-text-3">
          {t("transforms.yours.help")}
        </p>
        <ul className="grid gap-3 min-[760px]:grid-cols-2">
          {transforms.map((transform, i) => (
            <TransformCard
              key={transform.id}
              transform={transform}
              keys={keysOf(`transform:${transform.id}`)}
              model={modelLine(transform, providers, status, t)}
              usage={usageOf(runs?.usage, transform.id)}
              first={i === 0}
              last={i === transforms.length - 1}
              formatWhen={formatWhen}
              onEdit={() => setEditing(transform)}
              onToggle={async (on) =>
                report(await commands.setTransformEnabled(transform.id, on))
              }
              onMove={async (delta) =>
                report(await commands.moveTransform(transform.id, i + delta))
              }
              onDelete={() => remove(transform)}
              style={grid.shift(i, transform.id)}
              dragging={grid.dragging === transform.id}
              dragHandlers={grid.handlers(transform.id)}
            />
          ))}
        </ul>
      </section>

      <SettingsGroup title={t("transforms.after.title")}>
        <SettingContainer
          title={t("transforms.after.run.title")}
          description={
            autoId
              ? t("transforms.after.run.helpOn")
              : t("transforms.after.run.helpOff")
          }
        >
          <Dropdown
            className="w-[200px]"
            ariaLabel={t("transforms.after.run.title")}
            options={[
              { value: NONE, label: t("transforms.after.none") },
              ...options,
            ]}
            selectedValue={autoId ?? NONE}
            onSelect={(value) =>
              updateSetting("auto_transform_id", value === NONE ? null : value)
            }
          />
        </SettingContainer>
        <Requires met={!!autoId} reason={t("transforms.after.requires")}>
          <SettingContainer
            title={t("transforms.after.skipCode.title")}
            description={t("transforms.after.skipCode.help")}
          >
            <RowSwitch
              label={t("transforms.after.skipCode.title")}
              checked={getSetting("auto_transform_skip_code") ?? true}
              onChange={(on) => updateSetting("auto_transform_skip_code", on)}
            />
          </SettingContainer>
          <SettingContainer
            title={t("transforms.after.skipAi.title")}
            description={t("transforms.after.skipAi.help")}
          >
            <RowSwitch
              label={t("transforms.after.skipAi.title")}
              checked={getSetting("auto_transform_skip_ai") ?? true}
              onChange={(on) => updateSetting("auto_transform_skip_ai", on)}
            />
          </SettingContainer>
          <SettingContainer
            title={t("transforms.after.minWords.title")}
            description={t("transforms.after.minWords.help")}
          >
            <input
              type="range"
              className="slider w-40"
              aria-label={t("transforms.after.minWords.title")}
              aria-valuetext={t("transforms.after.minWords.value", {
                count: minWords,
              })}
              min={0}
              max={30}
              step={1}
              value={minWords}
              onChange={(e) =>
                updateSetting(
                  "auto_transform_min_words",
                  Number(e.target.value),
                )
              }
            />
            <span className="w-[72px] text-end text-[13px] tabular-nums text-text-2">
              {t("transforms.after.minWords.value", { count: minWords })}
            </span>
          </SettingContainer>
        </Requires>
        <SettingContainer
          title={t("transforms.after.dictate.title")}
          description={
            dictateKeys
              ? t("transforms.after.dictate.help", { keys: dictateKeys })
              : t("transforms.after.dictate.helpNoKeys")
          }
        >
          <Dropdown
            className="w-[200px]"
            ariaLabel={t("transforms.after.dictate.title")}
            options={options}
            selectedValue={dictateId}
            onSelect={(value) => updateSetting("dictate_transform_id", value)}
          />
        </SettingContainer>
      </SettingsGroup>

      {usable.length > 0 && (
        <section className="mb-8">
          <h2 className="mb-3 text-[17px] font-semibold tracking-[-0.01em]">
            {t("transforms.try.title")}
          </h2>
          <TryIt transforms={usable} providers={providers} status={status} />
        </section>
      )}

      <RecentRuns runs={runs?.runs ?? []} viewKeys={keysOf("view_changes")} />

      {editing && (
        <TransformEditor
          key={editing.id || "new"}
          transform={editing}
          providers={providers}
          status={status}
          onClose={() => setEditing(null)}
          onDelete={(transform) => {
            setEditing(null);
            remove(transform);
          }}
          onSaved={(transform, created) => {
            setEditing(null);
            toast.success(
              created
                ? t("transforms.toast.created", { name: transform.name })
                : t("transforms.toast.saved", { name: transform.name }),
            );
          }}
        />
      )}
    </div>
  );
};
