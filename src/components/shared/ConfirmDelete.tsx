import React, { useRef } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";

/** Asks before something is deleted. Enter deletes, Esc keeps it. */
export const ConfirmDelete: React.FC<{
  open: boolean;
  title: string;
  description: string;
  confirmLabel?: string;
  onConfirm: () => void;
  onCancel: () => void;
}> = ({ open, title, description, confirmLabel, onConfirm, onCancel }) => {
  const { t } = useTranslation();
  const confirm = useRef<HTMLButtonElement>(null);
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => !next && onCancel()}
      title={title}
      description={description}
      closeLabel={t("common.cancel")}
      initialFocusRef={confirm as React.RefObject<HTMLElement>}
      footer={
        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={onCancel}>
            {t("common.cancel")}
          </Button>
          <Button ref={confirm} variant="danger" onClick={onConfirm}>
            {confirmLabel ?? t("common.delete")}
          </Button>
        </div>
      }
    />
  );
};
