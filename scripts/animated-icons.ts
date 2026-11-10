// Brings Legato's hover-animated icons up to date with pqoqubbw/icons, the
// MIT-licensed set of Lucide icons animated one by one with Motion.
//
//   bun scripts/animated-icons.ts            fetches the pinned commit
//   bun scripts/animated-icons.ts <folder>   reads <folder>/<name>.tsx
//
// Each icon's source is kept as written, apart from what makes it drop in
// where a Lucide icon was: the wrapping <div> goes, so the <svg> is the
// root as in lucide-react, and the svg takes the class, the size and a ref
// that Legato's hover trigger needs (see ../src/components/icons/animated).

import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

/** The commit of pqoqubbw/icons the set was taken from. */
const COMMIT = "4662fff2b60fdcf4751b32013606756caf7c4c26";

/** Lucide's name for an icon, then the set's file when it has another. */
const ICONS: [lucide: string, file?: string][] = [
  ["arrow-down"],
  ["arrow-down-left"],
  ["arrow-down-right"],
  ["arrow-right"],
  ["arrow-up"],
  ["arrow-up-right"],
  ["audio-lines"],
  ["blocks"],
  ["bluetooth"],
  ["bluetooth-off"],
  ["bold"],
  ["bot"],
  ["calendar-days"],
  ["check"],
  ["check-check"],
  ["chevron-down"],
  ["chevron-left"],
  ["chevron-right"],
  ["chevron-up"],
  ["circle-question-mark", "circle-help"],
  ["copy"],
  ["corner-down-left"],
  ["cpu"],
  ["download"],
  ["external-link"],
  ["eye"],
  ["eye-off"],
  ["file-text"],
  ["folder-input"],
  ["folder-plus"],
  ["history"],
  ["house", "home"],
  ["italic"],
  ["keyboard"],
  ["languages"],
  ["maximize-2"],
  ["message-circle"],
  ["mic"],
  ["mic-off"],
  ["panel-left-close"],
  ["panel-left-open"],
  ["pause"],
  ["phone-incoming"],
  ["play"],
  ["plus"],
  ["refresh-cw"],
  ["rotate-ccw"],
  ["rotate-cw"],
  ["search"],
  ["send"],
  ["settings"],
  ["shield-check"],
  ["square-pen"],
  ["trash-2", "delete"],
  ["upload"],
  ["users"],
  ["wifi"],
  ["x"],
];

const OUT = path.join(import.meta.dir, "../src/components/icons/animated/set");

const header = (
  file: string,
) => `// From pqoqubbw/icons (MIT, © pqoqubbw), icons/${file}.tsx at ${COMMIT.slice(0, 8)},
// adapted by scripts/animated-icons.ts: the svg is the root, as in
// lucide-react. Don't edit by hand; change the script and run it again.

`;

/** The index of the `>` that closes the tag opening at `from`. */
const tagEnd = (source: string, from: number): number => {
  let depth = 0;
  for (let i = from; i < source.length; i++) {
    const c = source[i];
    if (c === "{") depth++;
    else if (c === "}") depth--;
    else if (c === ">" && depth === 0) return i;
  }
  throw new Error("An svg tag never closes");
};

const adapt = (source: string, lucide: string, file: string): string => {
  const must = (what: string, found: boolean) => {
    if (!found) throw new Error(`${file}: ${what} isn't where it was`);
  };
  let s = source.replace(/^"use client";\s*/, "");

  must("the cn import", s.includes('from "@/lib/utils"'));
  s = s.replace('from "@/lib/utils"', 'from "../cn"');

  must("the props type", s.includes("HTMLAttributes<HTMLDivElement>"));
  s = s
    .replaceAll(
      "HTMLAttributes<HTMLDivElement>",
      "SVGAttributes<SVGSVGElement>",
    )
    .replace(/\bHTMLAttributes\b(?=[^;]*from "react")/, "SVGAttributes")
    .replaceAll("MouseEvent<HTMLDivElement>", "MouseEvent<SVGSVGElement>");
  if (!/import type React\b/.test(s))
    s = s.replace(
      /^(import [^;]*from "react";)/m,
      'import type React from "react";\n$1',
    );

  must("the size prop", /\bsize\?: number;/.test(s) && s.includes("size = 28"));
  s = s
    .replace(
      /\bsize\?: number;/,
      "size?: number;\n  /** The svg element, for Legato's hover trigger. */\n  rootRef?: React.Ref<SVGSVGElement>;",
    )
    .replace("size = 28", "size = 24, rootRef");

  const wrapper =
    /<div\s+className=\{cn\(className\)\}\s+onMouseEnter=\{handleMouseEnter\}\s+onMouseLeave=\{handleMouseLeave\}\s+\{\.\.\.props\}\s*>\s*<(motion\.svg|svg)/;
  const open = s.match(wrapper);
  must("the wrapping div", !!open && open.index !== undefined);
  const tag = open![1];
  const start = open!.index!;
  const afterName = start + open![0].length;
  const end = tagEnd(s, afterName);
  // A class the set gives the svg itself joins Legato's.
  let attrs = s.slice(afterName, end);
  const own = attrs.match(/\sclassName="([^"]*)"/);
  if (own) attrs = attrs.replace(own[0], "");
  const classes = `lucide lucide-${lucide}${own ? ` ${own[1]}` : ""}`;
  // What the caller passes goes last, so a strokeWidth of theirs wins.
  const rest =
    tag === "svg" ? "{...props}" : "{...(props as Record<string, unknown>)}";
  s =
    s.slice(0, start) +
    `<${tag}\n className={cn("${classes}", className)}\n onMouseEnter={handleMouseEnter}\n onMouseLeave={handleMouseLeave}\n ref={rootRef}` +
    attrs +
    `\n ${rest}\n` +
    s.slice(end);

  const close = s.lastIndexOf("</div>");
  must("the closing div", close >= 0 && s.indexOf("</div>") === close);
  s = s.slice(0, close) + s.slice(close + "</div>".length);
  return header(file) + s;
};

/** The name a set file exports its icon under. */
const exported = (source: string, file: string): string => {
  const name = source.match(/export \{\s*(?:\w+ as )?(\w+Icon)\s*\}/)?.[1];
  if (!name) throw new Error(`${file}: no icon is exported`);
  return name;
};

const read = async (file: string, folder?: string): Promise<string> => {
  if (folder) return readFile(path.join(folder, `${file}.tsx`), "utf8");
  const url = `https://raw.githubusercontent.com/pqoqubbw/icons/${COMMIT}/icons/${file}.tsx`;
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url}: ${response.status}`);
  return response.text();
};

const pascal = (name: string) =>
  name
    .split("-")
    .map((part) => part[0].toUpperCase() + part.slice(1))
    .join("");

const folder = process.argv[2];
await mkdir(OUT, { recursive: true });
const lines: string[] = [];
for (const [lucide, other] of ICONS) {
  const file = other ?? lucide;
  const source = await read(file, folder);
  await writeFile(path.join(OUT, `${lucide}.tsx`), adapt(source, lucide, file));
  lines.push(
    `export { ${exported(source, file)} as ${pascal(lucide)}Set } from "./${lucide}";`,
  );
}
await writeFile(
  path.join(OUT, "index.ts"),
  `// Written by scripts/animated-icons.ts: the icons taken from pqoqubbw/icons.\n\n${lines.join("\n")}\n`,
);
console.log(
  `${ICONS.length} icons written to ${path.relative(process.cwd(), OUT)}`,
);

// src/icons.tsx: every Lucide icon as it is, and the ones that move in
// place of their still versions, under each name lucide-react has for them.
const { MOVES } = await import("../src/components/icons/animated/craft");
const lucideIndex = await readFile(
  path.join(
    import.meta.dir,
    "../node_modules/lucide-react/dist/esm/lucide-react.js",
  ),
  "utf8",
);
const namesOf = (file: string): string[] => {
  const line = lucideIndex
    .split("\n")
    .find((l) => l.endsWith(`from './icons/${file}.js';`));
  if (!line) throw new Error(`lucide-react has no ${file}`);
  return [...line.matchAll(/default as (\w+)/g)]
    .map((m) => m[1])
    .filter((name) => !name.startsWith("Lucide"));
};
const setFiles = new Map(
  ICONS.map(([lucide, other]) => [lucide, other ?? lucide]),
);
const imports: string[] = [];
const consts: string[] = [];
const names: string[] = [];
for (const lucide of [
  ...new Set([...setFiles.keys(), ...Object.keys(MOVES)]),
].sort()) {
  const all = namesOf(lucide);
  const still = `${all[0]}Still`;
  const moving = `${all[0]}Moving`;
  imports.push(`  ${all[0]} as ${still},`);
  if (setFiles.has(lucide)) {
    const file = setFiles.get(lucide)!;
    const icon = exported(await read(file, folder), file);
    consts.push(
      `const ${moving} = hover(${still}, () =>\n  import("./components/icons/animated/set/${lucide}").then((m) => m.${icon}),\n);`,
    );
  } else {
    consts.push(
      `const ${moving} = hover(\n  ${still},\n  crafted("${lucide}", () => import("lucide-react/dist/esm/icons/${lucide}")),\n);`,
    );
  }
  names.push(...all.map((name) => `  ${moving} as ${name},`));
}
await writeFile(
  path.join(import.meta.dir, "../src/icons.tsx"),
  `// Written by scripts/animated-icons.ts. Import icons from here, not from
// lucide-react: the ones listed below move when the control they sit in is
// hovered, and every other Lucide icon comes through as it is.

import {
${imports.join("\n")}
} from "lucide-react";
import { crafted, hover } from "./components/icons/animated/hover";

export * from "lucide-react";

${consts.join("\n")}

export {
${names.join("\n")}
};
`,
);
console.log(`src/icons.tsx: ${consts.length} icons that move`);
