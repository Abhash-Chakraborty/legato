import React from "react";
import { useTranslation } from "react-i18next";
import ReactMarkdown, { type Components } from "react-markdown";
import { openUrl } from "@tauri-apps/plugin-opener";

const ALLOWED = [
  "a",
  "blockquote",
  "br",
  "code",
  "em",
  "h1",
  "h2",
  "h3",
  "h4",
  "hr",
  "li",
  "ol",
  "p",
  "pre",
  "strong",
  "ul",
];

const open = (href: string | undefined) => {
  if (!href) return;
  try {
    const url = new URL(href);
    if (["http:", "https:", "mailto:"].includes(url.protocol))
      openUrl(href).catch(() => {});
  } catch {
    // Not a link Legato opens.
  }
};

const components: Components = {
  h1: ({ children }) => (
    <h3 className="mb-2 mt-1 text-xl font-semibold tracking-[-0.01em]">
      {children}
    </h3>
  ),
  h2: ({ children }) => (
    <h4 className="mb-1.5 mt-1 text-[17px] font-semibold tracking-[-0.01em]">
      {children}
    </h4>
  ),
  h3: ({ children }) => (
    <h5 className="mb-1 mt-1 text-[15px] font-semibold">{children}</h5>
  ),
  h4: ({ children }) => (
    <h5 className="mb-1 mt-1 text-[15px] font-semibold">{children}</h5>
  ),
  p: ({ children }) => <p className="mb-2.5">{children}</p>,
  ul: ({ children }) => (
    <ul className="mb-2.5 list-disc ps-[22px]">{children}</ul>
  ),
  ol: ({ children }) => (
    <ol className="mb-2.5 list-decimal ps-[22px]">{children}</ol>
  ),
  li: ({ children }) => (
    <li className="my-0.5 ps-0.5 marker:text-text-3">{children}</li>
  ),
  blockquote: ({ children }) => (
    <blockquote className="mb-2.5 border-s-[3px] border-line-strong ps-3 text-text-2">
      {children}
    </blockquote>
  ),
  code: ({ children }) => (
    <code className="rounded bg-surface-3 px-[5px] py-px font-mono text-[0.88em]">
      {children}
    </code>
  ),
  pre: ({ children }) => (
    <pre className="mb-2.5 overflow-x-auto whitespace-pre rounded-[var(--r-sm)] bg-surface-3 p-3 text-[13px]">
      {children}
    </pre>
  ),
  hr: () => <hr className="my-3 border-line" />,
  a: ({ children, href }) => (
    <a
      href={href}
      onClick={(e) => {
        e.preventDefault();
        open(href);
      }}
      className="text-teal-700 underline underline-offset-2"
    >
      {children}
    </a>
  ),
};

/** A note's Markdown, read-only. */
export const NotePreview: React.FC<{ text: string; className?: string }> = ({
  text,
  className = "",
}) => {
  const { t } = useTranslation();
  return (
    <div
      className={`overflow-auto text-[15px] leading-[1.6] text-text ${className}`}
    >
      {text.trim() ? (
        <ReactMarkdown
          allowedElements={ALLOWED}
          unwrapDisallowed
          components={components}
        >
          {text}
        </ReactMarkdown>
      ) : (
        <p className="text-text-4">{t("scratchpad.editor.previewEmpty")}</p>
      )}
    </div>
  );
};
