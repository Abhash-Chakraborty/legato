import React, { useMemo } from "react";
import { wordDiff } from "../../lib/utils/diff";

/** A word diff: added words on a teal wash, removed words struck through. */
export const DiffText: React.FC<{
  before: string;
  after: string;
  className?: string;
}> = ({ before, after, className = "" }) => {
  const parts = useMemo(() => wordDiff(before, after), [before, after]);
  return (
    <p
      className={`whitespace-pre-wrap break-words leading-relaxed ${className}`}
    >
      {parts.map((part, i) =>
        part.kind === "same" ? (
          <span key={i}>{part.text}</span>
        ) : part.kind === "added" ? (
          <ins key={i} className="rounded-[3px] bg-teal-50 no-underline">
            {part.text}
          </ins>
        ) : (
          <del key={i} className="text-text-3 decoration-danger/70">
            {part.text}
          </del>
        ),
      )}
    </p>
  );
};
