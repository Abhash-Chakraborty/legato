import React, { useEffect, useState } from "react";
import { AppWindow } from "lucide-react";
import { commands } from "@/bindings";

// One lookup per executable; the backend caches too, this saves the IPC.
const icons = new Map<string, Promise<string | null>>();

const iconFor = (path: string): Promise<string | null> => {
  let icon = icons.get(path);
  if (!icon) {
    icon = commands.getAppIcon(path).catch(() => null);
    icons.set(path, icon);
  }
  return icon;
};

/** The app's own icon from its executable, or a plain window glyph. */
export const AppIcon: React.FC<{ path: string | null; className?: string }> = ({
  path,
  className = "h-4 w-4",
}) => {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    setSrc(null);
    if (path) iconFor(path).then((icon) => live && setSrc(icon));
    return () => {
      live = false;
    };
  }, [path]);

  if (src) {
    return (
      <img
        src={src}
        alt=""
        aria-hidden="true"
        draggable={false}
        className={`${className} flex-none object-contain`}
      />
    );
  }
  return (
    <AppWindow
      aria-hidden="true"
      className={`${className} flex-none stroke-[1.75] text-text-3`}
    />
  );
};
