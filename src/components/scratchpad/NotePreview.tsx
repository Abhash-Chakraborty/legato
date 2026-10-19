import React, { createContext, useContext, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Check } from "lucide-react";
import ReactMarkdown, { type Components } from "react-markdown";
import { openUrl } from "@tauri-apps/plugin-opener";
import { toggleTask } from "./notes";

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
  "input",
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

/** The bits of a Markdown tree the task boxes need. */
interface MdNode {
  type: string;
  value?: string;
  checked?: boolean | null;
  children?: MdNode[];
}

const TASK = /^\[( |x|X)\]\s+/;

/** "- [ ] milk" and "- [x] milk" become task boxes, without GitHub's whole
 *  Markdown flavour. */
const remarkTasks = () => (tree: MdNode) => {
  const walk = (node: MdNode) => {
    if (node.type === "listItem") {
      const text = node.children?.[0]?.children?.[0];
      const match =
        node.children?.[0]?.type === "paragraph" && text?.type === "text"
          ? TASK.exec(text.value ?? "")
          : null;
      if (match && text) {
        node.checked = match[1] !== " ";
        text.value = (text.value ?? "").slice(match[0].length);
      }
    }
    node.children?.forEach(walk);
  };
  walk(tree);
};

/** Where the list item being drawn starts in the note, for its task box. */
const TaskItem = createContext<number | null>(null);

const TaskBox: React.FC<{
  checked: boolean;
  onToggle?: (offset: number) => void;
}> = ({ checked, onToggle }) => {
  const { t } = useTranslation();
  const offset = useContext(TaskItem);
  const box = `me-2 inline-grid h-4 w-4 translate-y-[2px] place-items-center rounded-[4px] border-[1.5px] align-baseline transition-colors duration-[var(--dur-1)] ${
    checked
      ? "border-teal-600 bg-teal-600 text-white"
      : "border-line-strong bg-surface"
  }`;
  const mark = checked && (
    <Check className="h-3 w-3" strokeWidth={3} aria-hidden="true" />
  );
  if (!onToggle || offset === null)
    return (
      <span
        role="checkbox"
        aria-checked={checked}
        aria-disabled="true"
        className={box}
      >
        {mark}
      </span>
    );
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={checked}
      aria-label={
        checked ? t("scratchpad.preview.untick") : t("scratchpad.preview.tick")
      }
      onClick={() => onToggle(offset)}
      className={`${box} cursor-pointer hover:border-teal-600 active:scale-[0.92]`}
    >
      {mark}
    </button>
  );
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
  li: ({ children, node, className }) =>
    className?.includes("task-list-item") ? (
      <TaskItem.Provider value={node?.position?.start.offset ?? null}>
        <li className="my-0.5 -ms-[18px] list-none ps-0.5">{children}</li>
      </TaskItem.Provider>
    ) : (
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

/** A note's Markdown, read-only apart from its task boxes, which tick when
 *  `onChange` is given. */
export const NotePreview: React.FC<{
  text: string;
  className?: string;
  onChange?: (text: string) => void;
}> = ({ text, className = "", onChange }) => {
  const { t } = useTranslation();
  const all = useMemo<Components>(
    () => ({
      ...components,
      input: ({ checked }) => (
        <TaskBox
          checked={!!checked}
          onToggle={
            onChange && ((offset) => onChange(toggleTask(text, offset)))
          }
        />
      ),
    }),
    [onChange, text],
  );
  return (
    <div
      className={`overflow-auto text-[15px] leading-[1.6] text-text ${className}`}
    >
      {text.trim() ? (
        <ReactMarkdown
          allowedElements={ALLOWED}
          unwrapDisallowed
          remarkPlugins={[remarkTasks]}
          components={all}
        >
          {text}
        </ReactMarkdown>
      ) : (
        <p className="text-text-4">{t("scratchpad.editor.previewEmpty")}</p>
      )}
    </div>
  );
};
