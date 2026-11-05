import React from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { commands, type MergeSuggestion } from "@/bindings";
import { Button } from "../../ui/Button";
import { mergeWords } from "./merge";

/** Words that look like one ("Abhash" and "Abhash Chakraborty"), each
 *  with Merge, which keeps the second as a way the first is also written,
 *  and Keep apart, which stops offering it. */
export const MergeList: React.FC<{ merges: MergeSuggestion[] }> = ({
  merges,
}) => {
  const { t } = useTranslation();
  const apart = async (m: MergeSuggestion) => {
    const result = await commands.keepDictionaryEntriesApart(
      m.keep.text,
      m.other.text,
    );
    if (result.status === "error") toast.error(result.error);
  };
  return (
    <ul className="overflow-hidden rounded-[var(--r-lg)] border border-line bg-surface">
      {merges.map((m) => (
        <li
          key={`${m.keep.id}-${m.other.id}`}
          className="flex items-center gap-4 border-b border-line px-4 py-3 last:border-b-0"
        >
          <div className="min-w-0 flex-1">
            <div className="truncate text-[15px]">
              <span className="font-medium">{m.keep.text}</span>
              <span className="text-text-3">
                {" "}
                {t("dictionary.merges.and")}{" "}
              </span>
              <span className="font-medium">{m.other.text}</span>
            </div>
            <div className="mt-0.5 truncate text-[13px] text-text-3">
              {t("dictionary.merges.result", {
                word: m.keep.text,
                other: m.other.text,
              })}
            </div>
          </div>
          <div className="flex flex-none items-center gap-1">
            <Button variant="ghost" size="sm" onClick={() => apart(m)}>
              {t("dictionary.merges.apart")}
            </Button>
            <Button
              variant="primary-soft"
              size="sm"
              onClick={() => mergeWords(t, m.keep, m.other)}
            >
              {t("dictionary.merges.merge")}
            </Button>
          </div>
        </li>
      ))}
    </ul>
  );
};
