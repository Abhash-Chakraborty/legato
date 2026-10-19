import React from "react";
import { useTranslation } from "react-i18next";
import { CornerDownLeft } from "lucide-react";
import { pieces } from "./list";

/** A snippet's text on one line: variables as chips, line breaks as ↵. */
export const SnippetText: React.FC<{ text: string }> = ({ text }) => {
  const { t } = useTranslation();
  return (
    <span data-snippet-text className="truncate">
      {pieces(text).map((p, i) =>
        p.kind === "break" ? (
          <CornerDownLeft
            key={i}
            role="img"
            aria-label={t("snippets.row.lineBreak")}
            className="mx-1.5 inline h-3.5 w-3.5 align-[-2px] text-text-4"
          />
        ) : p.kind === "variable" ? (
          <code
            key={i}
            className="mx-0.5 rounded-[var(--r-xs)] bg-surface-3 px-1 py-px font-mono text-[12px] text-text-2"
          >
            {`{{${p.name}}}`}
          </code>
        ) : (
          <React.Fragment key={i}>{p.text}</React.Fragment>
        ),
      )}
    </span>
  );
};
