import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands } from "@/bindings";

/** What Correct opens: the last paste and the app it went into. */
export interface CorrectPayload {
  text: string;
  app: string | null;
  /** What the speech model heard, when it differs from the paste. */
  heard: string | null;
}

/** Correct last dictation: the pasted text to fix. Enter puts the fix in
 *  place of the paste and teaches the Dictionary; Esc leaves it as it was. */
export const CorrectCard: React.FC<{ card: CorrectPayload }> = ({ card }) => {
  const { t } = useTranslation();
  const [text, setText] = useState(card.text);
  const [showHeard, setShowHeard] = useState(false);
  const ref = useRef<HTMLTextAreaElement>(null);

  // The bar takes the keyboard only now, so the field asks again once the
  // window has it.
  useEffect(() => {
    setText(card.text);
    const focus = () => {
      const field = ref.current;
      if (!field || document.activeElement === field) return;
      field.focus();
      field.setSelectionRange(field.value.length, field.value.length);
    };
    focus();
    // A click into another app closes the editor, leaving the focus there.
    let had = false;
    const onFocus = () => {
      had = true;
      focus();
    };
    const onBlur = () => {
      if (had) commands.barCorrectCancel(false);
    };
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    if (document.hasFocus()) had = true;
    return () => {
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
    };
  }, [card]);

  const changed = text.trim() !== card.text.trim() && text.trim() !== "";
  const apply = () =>
    changed ? commands.barCorrectApply(text) : commands.barCorrectCancel(true);

  return (
    <div
      className="scard panel scorrect"
      role="dialog"
      aria-label={t("overlay.correct.title")}
    >
      <div className="spanel-head">
        <span className="spanel-title">
          {card.app
            ? t("overlay.correct.titleIn", { app: card.app })
            : t("overlay.correct.title")}
        </span>
        <span className="spanel-keys">{t("overlay.correct.keys")}</span>
      </div>
      <textarea
        ref={ref}
        className="scorrect-text"
        value={text}
        rows={3}
        spellCheck
        aria-label={t("overlay.correct.title")}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            apply();
          } else if (e.key === "Escape") {
            e.preventDefault();
            commands.barCorrectCancel(true);
          }
        }}
      />
      {card.heard && showHeard && (
        <p className="scorrect-heard">
          <span className="sdiff-label">{t("overlay.correct.heardLabel")}</span>{" "}
          {card.heard}
        </p>
      )}
      <div className="spanel-actions">
        {card.heard ? (
          <button
            type="button"
            className="sbtn quiet scorrect-toggle"
            aria-expanded={showHeard}
            onClick={() => {
              setShowHeard(!showHeard);
              ref.current?.focus();
            }}
          >
            {showHeard
              ? t("overlay.correct.hideHeard")
              : t("overlay.correct.showHeard")}
          </button>
        ) : (
          <span className="spanel-note">{t("overlay.correct.note")}</span>
        )}
        <button
          type="button"
          className="sbtn quiet"
          onClick={() => commands.barCorrectCancel(true)}
        >
          {t("overlay.correct.cancel")}
        </button>
        <button
          type="button"
          className="sbtn primary"
          disabled={!changed}
          onClick={apply}
        >
          {t("overlay.correct.apply")}
        </button>
      </div>
    </div>
  );
};
