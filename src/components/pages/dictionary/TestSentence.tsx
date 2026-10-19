import React, { useEffect, useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands, type DictionaryTest, type EntryInput } from "@/bindings";
import { markFixes } from "../../../lib/utils/words";

interface TestSentenceProps {
  /** A word being edited, tested before it is saved. */
  draft?: EntryInput | null;
  draftId?: number | null;
  label: string;
  placeholder: string;
  inputRef?: React.Ref<HTMLInputElement>;
}

/** Type how something sounds and see what the Dictionary pastes, with the
 *  corrected words marked. Runs the pipeline's own correction. */
export const TestSentence: React.FC<TestSentenceProps> = ({
  draft = null,
  draftId = null,
  label,
  placeholder,
  inputRef,
}) => {
  const { t } = useTranslation();
  const id = useId();
  const [text, setText] = useState("");
  const [result, setResult] = useState<DictionaryTest | null>(null);
  const draftKey = JSON.stringify(draft);

  useEffect(() => {
    if (!text.trim()) {
      setResult(null);
      return;
    }
    let live = true;
    const timer = setTimeout(async () => {
      const r = await commands
        .testDictionary(text, draft, draftId)
        .catch(() => null);
      if (live && r?.status === "ok") setResult(r.data);
    }, 120);
    return () => {
      live = false;
      clearTimeout(timer);
    };
    // draftKey stands in for draft, which is a new object on every render.
  }, [text, draftKey, draftId]);

  return (
    <div>
      <label htmlFor={id} className="mb-1.5 block text-[13px] font-medium">
        {label}
      </label>
      <input
        id={id}
        ref={inputRef}
        className="input w-full"
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder={placeholder}
        autoComplete="off"
        spellCheck={false}
      />
      <p
        aria-live="polite"
        className="mt-2 min-h-[22px] text-sm leading-relaxed text-text-2"
      >
        {!result ? (
          <span className="text-[13px] text-text-3">
            {t("dictionary.test.empty")}
          </span>
        ) : result.fixes.length === 0 ? (
          <>
            {result.text}{" "}
            <span className="text-[13px] text-text-3">
              {t("dictionary.test.unchanged")}
            </span>
          </>
        ) : (
          markFixes(result.text, result.fixes).map((seg, i) =>
            seg.fix ? (
              <mark
                key={i}
                className="test-mark text-text"
                title={t("dictionary.test.fixedFrom", { from: seg.fix.from })}
              >
                {seg.text}
              </mark>
            ) : (
              <span key={i}>{seg.text}</span>
            ),
          )
        )}
      </p>
    </div>
  );
};
