import React, { useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { Braces, Check, ChevronDown, Trash2 } from "lucide-react";
import { toast } from "sonner";
import {
  commands,
  type DictionaryEntry,
  type Snippet,
  type SnippetInput,
  type SnippetTest,
} from "@/bindings";
import { ActionMenu } from "../../ui/ActionMenu";
import { Button } from "../../ui/Button";
import { Segmented } from "../../ui/Segmented";
import { Sheet } from "../../ui/Sheet";
import { relativeTime } from "../dictionary/list";
import { normalizeTrigger } from "./list";

interface SnippetEditorProps {
  /** The snippet to edit, or null to add one. */
  snippet: Snippet | null;
  snippets: Snippet[];
  words: DictionaryEntry[];
  maxTriggerChars: number;
  maxTextChars: number;
  variables: string[];
  onClose: () => void;
  onSaved: (snippet: Snippet, created: boolean) => void;
  onDelete: (snippet: Snippet) => void;
  onEditOther: (snippet: Snippet) => void;
}

/** A Dictionary word whose misheard form is in the trigger: the Dictionary
 *  fixes it first, so the trigger would never be heard as typed. */
const dictionaryClash = (trigger: string, words: DictionaryEntry[]) => {
  const said = ` ${normalizeTrigger(trigger)} `;
  if (!said.trim()) return null;
  for (const word of words) {
    const heard = word.heard.find((h) =>
      said.includes(` ${normalizeTrigger(h)} `),
    );
    if (heard) return { heard, word: word.text };
  }
  return null;
};

/** Add or edit one snippet: what you say, what gets pasted, whether it must
 *  be said on its own, and a test field that runs the real matcher. */
export const SnippetEditor: React.FC<SnippetEditorProps> = ({
  snippet,
  snippets,
  words,
  maxTriggerChars,
  maxTextChars,
  variables,
  onClose,
  onSaved,
  onDelete,
  onEditOther,
}) => {
  const { t, i18n } = useTranslation();
  const triggerId = useId();
  const textId = useId();
  const testId = useId();
  const triggerRef = useRef<HTMLInputElement>(null);
  const textRef = useRef<HTMLTextAreaElement>(null);
  const [trigger, setTrigger] = useState(snippet?.trigger ?? "");
  const [text, setText] = useState(snippet?.text ?? "");
  const [alone, setAlone] = useState(snippet?.alone ?? false);
  // The test follows the trigger until it is typed into.
  const [testText, setTestText] = useState<string | null>(null);
  const [test, setTest] = useState<SnippetTest | null>(null);
  const [menu, setMenu] = useState(false);
  const [error, setError] = useState<{
    field: "trigger" | "text";
    message: string;
  } | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => setError(null), [trigger, text]);

  const said = normalizeTrigger(trigger);
  const duplicate = said
    ? snippets.find(
        (s) => s.id !== snippet?.id && normalizeTrigger(s.trigger) === said,
      )
    : undefined;
  const clash = dictionaryClash(trigger, words);
  const draft: SnippetInput = { trigger: trigger.trim(), text, alone };
  const draftKey = JSON.stringify(draft);
  const testValue = testText ?? trigger;
  const draftId = snippet?.id ?? -1;

  useEffect(() => {
    if (!testValue.trim() || !said || !text.trim()) {
      setTest(null);
      return;
    }
    let live = true;
    const timer = setTimeout(async () => {
      const r = await commands
        .testSnippets(testValue, draft, draftId)
        .catch(() => null);
      if (live && r?.status === "ok") setTest(r.data);
    }, 120);
    return () => {
      live = false;
      clearTimeout(timer);
    };
    // draftKey stands in for draft, which is a new object on every render.
  }, [testValue, draftKey, draftId]);

  const insert = (variable: string) => {
    const area = textRef.current;
    const start = area?.selectionStart ?? text.length;
    const end = area?.selectionEnd ?? text.length;
    const next = text.slice(0, start) + variable + text.slice(end);
    if (next.length > maxTextChars) return;
    setText(next);
    requestAnimationFrame(() => {
      area?.focus();
      area?.setSelectionRange(start + variable.length, start + variable.length);
    });
  };

  const save = async () => {
    if (saving) return;
    if (!said) {
      setError({
        field: "trigger",
        message: t("snippets.editor.errors.noTrigger"),
      });
      triggerRef.current?.focus();
      return;
    }
    if (duplicate) {
      triggerRef.current?.focus();
      return;
    }
    if (!text.trim()) {
      setError({ field: "text", message: t("snippets.editor.errors.noText") });
      textRef.current?.focus();
      return;
    }
    setSaving(true);
    const result = snippet
      ? await commands.updateSnippet(snippet.id, draft)
      : await commands.addSnippet(draft);
    setSaving(false);
    if (result.status === "error") {
      setError({ field: "trigger", message: result.error });
      return;
    }
    onSaved(result.data, !snippet);
  };

  const now = Math.floor(Date.now() / 1000);
  const description = !snippet
    ? t("snippets.editor.addDescription")
    : snippet.uses === 0 || !snippet.last_used_at
      ? t("snippets.editor.notUsed")
      : snippet.last_app
        ? t("snippets.editor.usedIn", {
            count: snippet.uses,
            when: relativeTime(snippet.last_used_at, now, i18n.language),
            app: snippet.last_app,
          })
        : t("snippets.editor.used", {
            count: snippet.uses,
            when: relativeTime(snippet.last_used_at, now, i18n.language),
          });

  const matched = !!test && test.matched.includes(draftId);
  const triggerMessage = duplicate ? (
    <>
      {t("snippets.editor.errors.duplicate")}{" "}
      <button
        type="button"
        onClick={() => onEditOther(duplicate)}
        className="font-medium text-teal-700 underline-offset-2 hover:underline"
      >
        {t("snippets.editor.editThatOne")}
      </button>
    </>
  ) : error?.field === "trigger" ? (
    error.message
  ) : clash ? (
    t("snippets.editor.dictionaryClash", clash)
  ) : said ? (
    t("snippets.editor.triggerHelp", { trigger: trigger.trim() })
  ) : (
    t("snippets.editor.triggerEmpty")
  );

  return (
    <Sheet
      open
      title={
        snippet ? t("snippets.editor.editTitle") : t("snippets.editor.addTitle")
      }
      description={description}
      onClose={onClose}
      onSubmit={save}
      closeLabel={t("snippets.editor.cancel")}
      initialFocusRef={triggerRef}
      footer={
        <>
          {snippet && (
            <Button
              type="button"
              variant="danger-ghost"
              size="md"
              onClick={() => onDelete(snippet)}
              className="me-auto"
            >
              <Trash2 className="h-4 w-4" aria-hidden="true" />
              {t("snippets.editor.delete")}
            </Button>
          )}
          <Button
            type="button"
            variant="ghost"
            onClick={onClose}
            className={snippet ? "" : "ms-auto"}
          >
            {t("snippets.editor.cancel")}
          </Button>
          <Button type="submit" variant="primary" disabled={saving}>
            {t("snippets.editor.save")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-5">
        <div>
          <label
            htmlFor={triggerId}
            className="mb-1.5 block text-[13px] font-medium"
          >
            {t("snippets.editor.trigger")}
          </label>
          <input
            id={triggerId}
            ref={triggerRef}
            className={`input w-full ${duplicate || error?.field === "trigger" ? "invalid" : ""}`}
            value={trigger}
            maxLength={maxTriggerChars}
            onChange={(e) => setTrigger(e.target.value)}
            placeholder={t("snippets.editor.triggerPlaceholder")}
            autoComplete="off"
            spellCheck={false}
            aria-invalid={!!duplicate || error?.field === "trigger"}
            aria-describedby={`${triggerId}-msg`}
          />
          <p
            id={`${triggerId}-msg`}
            aria-live="polite"
            className={`mt-1.5 min-h-[18px] text-[13px] ${
              duplicate || error?.field === "trigger"
                ? "text-danger"
                : clash
                  ? "text-warn"
                  : "text-text-3"
            }`}
          >
            {triggerMessage}
          </p>
        </div>

        <div>
          <div className="mb-1.5 flex items-center justify-between gap-3">
            <label htmlFor={textId} className="block text-[13px] font-medium">
              {t("snippets.editor.text")}
            </label>
            <div className="relative">
              <button
                type="button"
                onClick={() => setMenu(!menu)}
                aria-haspopup="menu"
                aria-expanded={menu}
                className="inline-flex h-7 items-center gap-1.5 rounded-[var(--r-sm)] px-2 text-[13px] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:text-text hover:duration-0"
              >
                <Braces className="h-3.5 w-3.5" aria-hidden="true" />
                {t("snippets.editor.insertVariable")}
                <ChevronDown className="h-3.5 w-3.5" aria-hidden="true" />
              </button>
              {menu && (
                <ActionMenu
                  label={t("snippets.editor.insertVariable")}
                  onClose={() => setMenu(false)}
                  className="end-0 top-full z-10 mt-1 w-64"
                  items={variables.map((v) => ({
                    id: v,
                    label: t(
                      `snippets.variables.${v.replace(/[{}]/g, "").replace(":", "_")}`,
                      { variable: v },
                    ),
                    onSelect: () => insert(v),
                  }))}
                />
              )}
            </div>
          </div>
          <textarea
            id={textId}
            ref={textRef}
            className={`input min-h-[120px] w-full resize-y py-2 leading-relaxed ${error?.field === "text" ? "invalid" : ""}`}
            value={text}
            maxLength={maxTextChars}
            onChange={(e) => setText(e.target.value)}
            placeholder={t("snippets.editor.textPlaceholder")}
            spellCheck={false}
            aria-invalid={error?.field === "text"}
            aria-describedby={`${textId}-msg`}
          />
          <p
            id={`${textId}-msg`}
            aria-live="polite"
            className={`mt-1.5 text-[13px] ${error?.field === "text" ? "text-danger" : "text-text-3"}`}
          >
            {error?.field === "text"
              ? error.message
              : text.length > maxTextChars - 200
                ? t("snippets.editor.charsLeft", {
                    count: maxTextChars - text.length,
                  })
                : t("snippets.editor.textHelp")}
          </p>
        </div>

        <div>
          <span className="mb-1.5 block text-[13px] font-medium">
            {t("snippets.editor.expand")}
          </span>
          <Segmented
            label={t("snippets.editor.expand")}
            value={alone ? "alone" : "anywhere"}
            onChange={(v) => setAlone(v === "alone")}
            options={[
              { value: "anywhere", label: t("snippets.editor.anywhere") },
              { value: "alone", label: t("snippets.editor.alone") },
            ]}
          />
          <p className="mt-1.5 text-[13px] text-text-3">
            {alone
              ? t("snippets.editor.aloneHelp")
              : t("snippets.editor.anywhereHelp")}
          </p>
        </div>

        <div>
          <label
            htmlFor={testId}
            className="mb-1.5 block text-[13px] font-medium"
          >
            {t("snippets.editor.test")}
          </label>
          <input
            id={testId}
            className="input w-full"
            value={testValue}
            onChange={(e) => setTestText(e.target.value)}
            placeholder={t("snippets.editor.testPlaceholder")}
            autoComplete="off"
            spellCheck={false}
          />
          <div aria-live="polite" className="mt-2 min-h-[22px]">
            {!test ? (
              <span className="text-[13px] text-text-3">
                {t("snippets.editor.testEmpty")}
              </span>
            ) : (
              <>
                <p className="whitespace-pre-wrap break-words text-sm leading-relaxed text-text-2">
                  {test.text}
                </p>
                <p
                  className={`mt-1 inline-flex items-center gap-1 text-[13px] ${matched ? "text-teal-700" : "text-text-3"}`}
                >
                  {matched && (
                    <Check className="h-3.5 w-3.5" aria-hidden="true" />
                  )}
                  {matched
                    ? t("snippets.editor.expands")
                    : alone
                      ? t("snippets.editor.notAlone")
                      : t("snippets.editor.noMatch")}
                </p>
              </>
            )}
          </div>
        </div>
      </div>
    </Sheet>
  );
};

/** The toast after a save, with Undo for a new snippet. */
export const toastSaved = (
  t: TFunction,
  snippet: Snippet,
  created: boolean,
) => {
  if (!created) {
    toast.success(t("snippets.toast.saved", { trigger: snippet.trigger }));
    return;
  }
  toast.success(t("snippets.toast.added", { trigger: snippet.trigger }), {
    action: {
      label: t("snippets.toast.undo"),
      onClick: () => {
        commands.deleteSnippets([snippet.id]).catch(() => {});
      },
    },
  });
};
