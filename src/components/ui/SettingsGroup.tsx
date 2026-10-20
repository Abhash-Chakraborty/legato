import React from "react";

interface SettingsGroupProps {
  title?: string;
  description?: string;
  children: React.ReactNode;
}

/** A titled run of settings rows, separated by hairlines. */
export const SettingsGroup: React.FC<SettingsGroupProps> = ({
  title,
  description,
  children,
}) => {
  return (
    <section data-settings-group className="mb-5">
      {title && (
        <h2 className="mb-1 text-[17px] font-semibold tracking-[-0.01em]">
          {title}
        </h2>
      )}
      {description && (
        <p className="mb-2 text-[13px] text-text-3">{description}</p>
      )}
      <div className="divide-y divide-line">{children}</div>
    </section>
  );
};
