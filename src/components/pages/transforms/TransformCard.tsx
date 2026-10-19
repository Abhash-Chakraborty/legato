import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  ArrowDown,
  ArrowUp,
  EllipsisVertical,
  Keyboard,
  Pencil,
  Trash2,
} from "lucide-react";
import type { Transform, TransformUsage } from "@/bindings";
import { ActionMenu } from "../../ui/ActionMenu";
import { formatRunTime } from "./list";

export const KeyCaps: React.FC<{ keys: string[] }> = ({ keys }) => (
  <span className="inline-flex items-center gap-1">
    {keys.map((key, i) => (
      <kbd key={`${key}-${i}`} className="kbd">
        {key}
      </kbd>
    ))}
  </span>
);

interface TransformCardProps {
  transform: Transform;
  keys: string[];
  model: string;
  usage: TransformUsage | undefined;
  first: boolean;
  last: boolean;
  onEdit: () => void;
  onToggle: (enabled: boolean) => void;
  onMove: (delta: -1 | 1) => void;
  onDelete: () => void;
  formatWhen: (unixSeconds: number) => string;
  /** Offset while a card is dragged; see useGridDrag. */
  style?: React.CSSProperties;
  dragging?: boolean;
  dragHandlers?: React.HTMLAttributes<HTMLLIElement>;
}

/** One transform: its keys, switch, what it does, where it runs and how
 *  much it's used. The name opens the editor. */
export const TransformCard: React.FC<TransformCardProps> = ({
  transform,
  keys,
  model,
  usage,
  first,
  last,
  onEdit,
  onToggle,
  onMove,
  onDelete,
  formatWhen,
  style,
  dragging = false,
  dragHandlers,
}) => {
  const { t } = useTranslation();
  const [menu, setMenu] = useState(false);
  const enabled = transform.enabled !== false;
  const more = t("transforms.card.more", { name: transform.name });

  return (
    <li
      className={`group relative flex min-h-[172px] touch-none select-none flex-col rounded-[var(--r-lg)] border bg-surface p-4 transition-[border-color,transform] duration-[var(--dur-2)] hover:border-line-strong hover:duration-0 motion-reduce:transition-none ${
        dragging
          ? "cursor-grabbing border-line-strong shadow-[var(--shadow-1)]"
          : "cursor-grab border-line"
      }`}
      data-transform={transform.id}
      style={style}
      {...dragHandlers}
    >
      <div className="flex items-center justify-between gap-3">
        {keys.length > 0 ? (
          <KeyCaps keys={keys} />
        ) : (
          <button
            type="button"
            onClick={onEdit}
            className="inline-flex h-7 items-center gap-1.5 rounded-[var(--r-sm)] border border-dashed border-line-strong px-2.5 text-[13px] text-text-2 transition-colors duration-[var(--dur-1)] hover:border-teal-600 hover:text-text hover:duration-0"
          >
            <Keyboard className="h-3.5 w-3.5" aria-hidden="true" />
            {t("transforms.card.setShortcut")}
          </button>
        )}
        <div className="flex items-center gap-1">
          <div className="relative">
            <button
              type="button"
              onClick={() => setMenu(!menu)}
              aria-label={more}
              title={more}
              aria-haspopup="menu"
              aria-expanded={menu}
              className={`grid h-8 w-8 place-items-center rounded-[var(--r-sm)] text-text-3 transition-[color,background-color,opacity] duration-[var(--dur-1)] hover:bg-surface-2 hover:text-text hover:duration-0 focus-visible:opacity-100 group-focus-within:opacity-100 group-hover:opacity-100 ${menu ? "opacity-100" : "opacity-0"}`}
            >
              <EllipsisVertical className="h-4 w-4" aria-hidden="true" />
            </button>
            {menu && (
              <ActionMenu
                label={more}
                onClose={() => setMenu(false)}
                items={[
                  {
                    id: "edit",
                    label: t("transforms.card.edit"),
                    icon: Pencil,
                    onSelect: onEdit,
                  },
                  {
                    id: "up",
                    label: t("transforms.card.moveUp"),
                    icon: ArrowUp,
                    disabled: first,
                    onSelect: () => onMove(-1),
                  },
                  {
                    id: "down",
                    label: t("transforms.card.moveDown"),
                    icon: ArrowDown,
                    disabled: last,
                    onSelect: () => onMove(1),
                  },
                  ...(transform.built_in
                    ? []
                    : [
                        {
                          id: "delete",
                          label: t("transforms.card.delete"),
                          icon: Trash2,
                          danger: true,
                          onSelect: onDelete,
                        },
                      ]),
                ]}
              />
            )}
          </div>
          <button
            type="button"
            role="switch"
            aria-checked={enabled}
            aria-label={t("transforms.card.switch", { name: transform.name })}
            className="toggle"
            onClick={() => onToggle(!enabled)}
          />
        </div>
      </div>

      <button
        type="button"
        onClick={onEdit}
        className="mt-3 self-start text-start text-[15px] font-medium text-text underline-offset-2 hover:underline"
      >
        <span className={enabled ? "" : "text-text-2"}>{transform.name}</span>
      </button>
      {transform.description && (
        <p
          className={`mt-0.5 text-sm ${enabled ? "text-text-2" : "text-text-3"}`}
        >
          {transform.description}
        </p>
      )}

      <div className="mt-auto pt-4 text-xs leading-5 text-text-3">
        <p className="truncate">{model}</p>
        <p className="tabular-nums">
          {usage && usage.uses > 0
            ? t("transforms.card.used", {
                count: usage.uses,
                time: formatRunTime(usage.avg_ms),
                when: formatWhen(usage.last_at),
              })
            : t("transforms.card.unused")}
        </p>
      </div>
    </li>
  );
};
