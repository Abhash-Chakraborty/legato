// Builds legato.abhashchakraborty.tech into _site (or --out <dir>): the home page,
// the docs (docs/*.md), a docs home, a search index and a 404 page.
//
//   bun site/build.ts             links point at the latest release's files
//   bun site/build.ts --offline   no GitHub API call; links go to the release page
//
// The download links carry the version in their file names, so they are filled in from
// the latest release here. pages.yml runs this on pushes to main and after each release.
import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { createHash } from "node:crypto";
import { icon } from "./icons.ts";

const SITE = "https://legato.abhashchakraborty.tech/";
const REPO = "Abhash-Chakraborty/legato";
const GITHUB = `https://github.com/${REPO}`;
const LATEST = `${GITHUB}/releases/latest`;
const BLOB = `${GITHUB}/blob/main/`;

// The docs, in reading order; each page is docs/<slug>.md.
export const GROUPS: {
  title: string;
  pages: { slug: string; nav: string; icon: string }[];
}[] = [
  {
    title: "Start here",
    pages: [
      { slug: "getting-started", nav: "Getting started", icon: "play" },
      { slug: "install", nav: "Install on Windows", icon: "laptop" },
      { slug: "android", nav: "Legato for Android", icon: "smartphone" },
    ],
  },
  {
    title: "Using Legato",
    pages: [
      { slug: "dictation", nav: "Dictation", icon: "mic" },
      { slug: "dictionary", nav: "Dictionary and snippets", icon: "book-a" },
      { slug: "style", nav: "Style", icon: "sliders-horizontal" },
      { slug: "notes", nav: "Notes and meetings", icon: "notebook-pen" },
      { slug: "transforms", nav: "Transforms and Ask", icon: "square-pen" },
      {
        slug: "scratchpad",
        nav: "Home, Scratchpad and Insights",
        icon: "history",
      },
    ],
  },
  {
    title: "Settings and privacy",
    pages: [
      { slug: "settings", nav: "Settings", icon: "settings" },
      { slug: "connections", nav: "Connections", icon: "plug" },
      { slug: "privacy", nav: "Privacy and your data", icon: "shield-check" },
    ],
  },
  {
    title: "Reference",
    pages: [
      {
        slug: "troubleshooting",
        nav: "Troubleshooting",
        icon: "triangle-alert",
      },
      { slug: "verify", nav: "Verify a download", icon: "lock" },
      { slug: "cli", nav: "Command line", icon: "terminal" },
      { slug: "phone-sync", nav: "Phone sync protocol", icon: "wifi" },
    ],
  },
];
const DOCS = GROUPS.flatMap((g) =>
  g.pages.map((p) => ({ ...p, group: g.title })),
);

const root = join(import.meta.dir, "..");
const args = process.argv.slice(2);
const offline = args.includes("--offline");
const outArg = args.indexOf("--out");
const out = outArg >= 0 ? args[outArg + 1] : join(root, "_site");

type Asset = { name: string; size: number; browser_download_url: string };
type Release = {
  tag_name: string;
  published_at: string;
  html_url: string;
  assets: Asset[];
};

async function latestRelease(): Promise<Release | null> {
  if (offline) return null;
  const headers: Record<string, string> = {
    accept: "application/vnd.github+json",
  };
  if (process.env.GITHUB_TOKEN)
    headers.authorization = `Bearer ${process.env.GITHUB_TOKEN}`;
  const res = await fetch(
    `https://api.github.com/repos/${REPO}/releases/latest`,
    { headers },
  );
  if (!res.ok)
    throw new Error(`GitHub answered ${res.status} for the latest release`);
  return (await res.json()) as Release;
}

const esc = (s: string) =>
  s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");

const mb = (bytes: number) =>
  `${(bytes / 1024 / 1024).toFixed(bytes < 10 * 1024 * 1024 ? 1 : 0)} MB`;

/** A short hash of a built file, added to its URL so a new page never meets an old cached copy. */
const hashOf = (path: string) =>
  createHash("sha256").update(readFileSync(path)).digest("hex").slice(0, 10);

function downloads(rel: Release | null): Record<string, string> {
  const files: Record<string, RegExp> = {
    x64: /_x64-setup\.exe$/,
    arm64: /_arm64-setup\.exe$/,
    msi: /_x64_en-US\.msi$/,
    msiarm: /_arm64_en-US\.msi$/,
    apk: /^Legato-android-.+\.apk$/,
  };
  const v: Record<string, string> = {
    site: SITE,
    github: GITHUB,
    release: rel?.html_url ?? LATEST,
    version: rel ? esc(rel.tag_name.replace(/^v/, "")) : "latest",
    released: "",
    date: "",
  };
  if (rel) {
    v.date = new Date(rel.published_at).toLocaleDateString("en-GB", {
      day: "numeric",
      month: "long",
      year: "numeric",
      timeZone: "UTC",
    });
    v.released = `, released ${v.date}`;
  }
  for (const [key, re] of Object.entries(files)) {
    const a = rel?.assets.find((x) => re.test(x.name));
    if (rel && !a)
      throw new Error(`the latest release has no file matching ${re}`);
    v[key] = a ? a.browser_download_url : LATEST;
    v[`${key}-name`] = a ? esc(a.name) : "On the latest release";
    v[`${key}-size`] = a ? mb(a.size) : "";
  }
  const apk = rel?.assets.find((x) => files.apk.test(x.name));
  v["apk-version"] = apk
    ? esc(apk.name.replace(/^Legato-android-|\.apk$/g, ""))
    : "";
  return v;
}

/** Fills {{key}} from `v` and {{icon:name}} with an inline icon. */
function fill(html: string, v: Record<string, string>) {
  return html.replace(/\{\{([a-z0-9:-]+)\}\}/g, (m, k: string) => {
    if (k.startsWith("icon:")) return icon(k.slice(5));
    if (!(k in v)) throw new Error(`no value for ${m}`);
    return v[k];
  });
}

// ---------- shared chrome ----------

function head(o: {
  title: string;
  description: string;
  url: string;
  base: string;
  css: string;
  js: string;
}) {
  return `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>${esc(o.title)}</title>
    <meta name="description" content="${esc(o.description)}" />
    <link rel="canonical" href="${o.url}" />
    <meta property="og:title" content="${esc(o.title)}" />
    <meta property="og:description" content="${esc(o.description)}" />
    <meta property="og:url" content="${o.url}" />
    <meta property="og:image" content="${SITE}hero-light.webp" />
    <meta name="twitter:card" content="summary_large_image" />
    <meta name="theme-color" content="#f3f2ee" media="(prefers-color-scheme: light)" />
    <meta name="theme-color" content="#151615" media="(prefers-color-scheme: dark)" />
    <link rel="icon" href="${o.base}legato-icon.svg" type="image/svg+xml" />
    <link rel="preload" href="${o.base}fonts/figtree-latin.woff2" as="font" type="font/woff2" crossorigin />
    <link rel="stylesheet" href="${o.base}${o.css}" />
    <script>try{var t=localStorage.getItem("legato-theme");if(t)document.documentElement.dataset.theme=t}catch(e){}</script>
    <script src="${o.base}${o.js}" defer></script>
  </head>`;
}

function header(base: string, current: "home" | "docs") {
  const a = (href: string, label: string, cls = "") =>
    `<a href="${href}"${cls ? ` class="${cls}"` : ""}>${label}</a>`;
  return `<header class="top" data-top>
      <div class="top-in">
        <a class="brand" href="${base}" aria-label="Legato home"><img src="${base}legato-icon.svg" alt="" width="28" height="28" /><span>Legato</span></a>
        <nav class="top-nav" aria-label="Main">
          ${a(`${base}#features`, "Features")}
          ${a(`${base}#meetings`, "Meetings")}
          ${a(`${base}#phone`, "Phone")}
          <a href="${base}docs/"${current === "docs" ? ' aria-current="page"' : ""}>Docs</a>
        </nav>
        <div class="top-end">
          <button class="search-open" type="button" data-search-open aria-label="Search the docs (Ctrl+K)">${icon("search")}<span>Search</span><kbd>Ctrl K</kbd></button>
          <button class="icon-btn" type="button" data-theme-toggle aria-label="Switch between light and dark">${icon("sun", "icon i-sun")}${icon("moon", "icon i-moon")}</button>
          <a class="icon-btn hide-s" href="${GITHUB}" aria-label="Legato on GitHub">${icon("github")}</a>
          <a class="btn btn-sm hide-s" href="${base}#download">Download</a>
          <button class="icon-btn menu-btn" type="button" data-menu aria-expanded="false" aria-controls="sheet" aria-label="Menu">${icon("menu", "icon i-menu")}${icon("x", "icon i-x")}</button>
        </div>
      </div>
      <div class="sheet" id="sheet" hidden>
        ${a(`${base}#features`, "Features")}
        ${a(`${base}#meetings`, "Meetings")}
        ${a(`${base}#phone`, "Phone")}
        ${a(`${base}docs/`, "Docs")}
        ${a(GITHUB, "GitHub")}
        <a class="btn" href="${base}#download">${icon("download")}Download</a>
      </div>
    </header>`;
}

function footer(base: string, v: Record<string, string>) {
  const col = (title: string, links: [string, string][]) =>
    `<div><h2>${title}</h2>${links.map(([h, l]) => `<a href="${h}">${l}</a>`).join("")}</div>`;
  return `<footer class="foot">
      <div class="foot-in">
        <div class="foot-brand">
          <a class="brand" href="${base}"><img src="${base}legato-icon.svg" alt="" width="28" height="28" /><span>Legato</span></a>
          <p>Dictation and meeting notes for Windows, transcribed on your PC. Free and open source, MIT licence.</p>
          <p class="foot-ver">Version ${v.version}${v.date ? ` · ${v.date}` : ""}</p>
        </div>
        ${col("Get it", [
          [`${base}#download`, "Download"],
          [`${base}docs/install/`, "Install on Windows"],
          [`${base}docs/android/`, "Set up the phone"],
          [v.release, "What's new"],
        ])}
        ${col("Docs", [
          [`${base}docs/getting-started/`, "Getting started"],
          [`${base}docs/notes/`, "Notes and meetings"],
          [`${base}docs/privacy/`, "Privacy and your data"],
          [`${base}docs/troubleshooting/`, "Troubleshooting"],
        ])}
        ${col("Project", [
          [GITHUB, "Source on GitHub"],
          [`${GITHUB}/issues`, "Report a problem"],
          [`${BLOB}CONTRIBUTING.md`, "Contributing"],
          [`${BLOB}LICENSE`, "MIT licence"],
        ])}
      </div>
      <p class="foot-by">Legato by Abhash Chakraborty, built on <a href="https://github.com/cjpais/Handy">Handy</a>.</p>
    </footer>`;
}

const searchDialog = `<div class="search" data-search hidden>
      <div class="search-box" role="dialog" aria-modal="true" aria-label="Search the docs">
        <div class="search-field">${icon("search")}<input type="search" placeholder="Search the docs" aria-label="Search the docs" autocomplete="off" spellcheck="false" data-search-input /><kbd>Esc</kbd></div>
        <ul class="search-results" role="listbox" data-search-results></ul>
        <p class="search-empty" data-search-empty hidden>No pages match that. Try a shorter word.</p>
      </div>
    </div>`;

// ---------- docs ----------

// A link in a docs page, rewritten for where the page is served (docs/<slug>/).
function link(href: string, from: string): string {
  if (/^(https?:|mailto:|#)/.test(href)) return href;
  const [path, hash] = href.split("#");
  const doc = path.match(/^(?:\.\/)?([a-z0-9-]+)\.md$/);
  if (doc) return `../${doc[1]}/${hash ? `#${hash}` : ""}`;
  // anything else in the repo opens on GitHub
  const repoPath = join("docs", path).replace(/\\/g, "/");
  if (repoPath.startsWith(".."))
    throw new Error(`${from}.md links outside the repo: ${href}`);
  return BLOB + repoPath + (hash ? `#${hash}` : "");
}

const plain = (html: string) =>
  html
    // block ends become spaces, so table cells and list items don't run together
    .replace(/<\/?(?:p|li|td|th|tr|h[1-6]|br|div|blockquote|pre)\b[^>]*>/g, " ")
    .replace(/<[^>]+>/g, "")
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/\s+/g, " ")
    .trim();

type Heading = { id: string; text: string; level: number };
type Page = {
  slug: string;
  nav: string;
  group: string;
  icon: string;
  title: string;
  description: string;
  body: string;
  ids: Set<string>;
  headings: Heading[];
  sections: { id: string; heading: string; text: string }[];
};

function renderDoc(d: (typeof DOCS)[number]): Page {
  const md = readFileSync(join(root, "docs", `${d.slug}.md`), "utf8");
  let body = Bun.markdown.html(md, { headings: { ids: true } });
  const title = plain(body.match(/<h1[^>]*>(.*?)<\/h1>/s)?.[1] ?? d.nav);
  const first = plain(body.match(/<p>(.*?)<\/p>/s)?.[1] ?? "");
  const description =
    first.length > 160
      ? `${first.slice(0, first.lastIndexOf(" ", 157))}…`
      : first;
  body = body
    .replace(/<h1[^>]*>.*?<\/h1>\s*/s, "")
    .replace(/<table>/g, '<div class="table"><table>')
    .replace(/<\/table>/g, "</table></div>")
    .replace(
      /<blockquote>\s*<p><strong>Note:<\/strong>\s*/g,
      `<blockquote class="callout"><p>${icon("info", "icon callout-i")}`,
    )
    .replace(
      /<blockquote>\s*<p><strong>Warning:<\/strong>\s*/g,
      `<blockquote class="callout warn"><p>${icon("triangle-alert", "icon callout-i")}`,
    )
    .replace(
      /<pre><code([^>]*)>/g,
      `<div class="code"><button class="copy" type="button" data-copy aria-label="Copy">${icon("copy", "icon i-copy")}${icon("check", "icon i-check")}</button><pre><code$1>`,
    )
    .replace(/<\/code><\/pre>/g, "</code></pre></div>")
    .replace(
      /href="([^"]+)"/g,
      (_, h) => `href="${esc(link(h.replace(/&amp;/g, "&"), d.slug))}"`,
    )
    .replace(/<a href="(https?:[^"]+)">/g, '<a href="$1" rel="noopener">')
    .replace(
      /<h([23]) id="([^"]+)">(.*?)<\/h\1>/gs,
      (_, l, id, inner) =>
        `<h${l} id="${id}"><a class="anchor" href="#${id}" aria-label="Link to this section">#</a>${inner}</h${l}>`,
    );
  const headings: Heading[] = [
    ...body.matchAll(/<h([23]) id="([^"]+)">(.*?)<\/h\1>/gs),
  ].map((m) => ({
    level: Number(m[1]),
    id: m[2],
    text: plain(m[3].replace(/<a class="anchor"[^>]*>#<\/a>/, "")),
  }));
  // Search sections: the text under each h2/h3, and the lede under the title.
  const sections: Page["sections"] = [];
  const parts = body.split(/(?=<h[23] id=")/);
  for (const part of parts) {
    const h = part.match(/^<h[23] id="([^"]+)">(.*?)<\/h[23]>/s);
    const text = plain(part.replace(/^<h[23][^>]*>.*?<\/h[23]>/s, "")).slice(
      0,
      600,
    );
    sections.push({
      id: h ? h[1] : "",
      heading: h
        ? plain(h[2].replace(/<a class="anchor"[^>]*>#<\/a>/, ""))
        : title,
      text,
    });
  }
  const ids = new Set([...body.matchAll(/ id="([^"]+)"/g)].map((m) => m[1]));
  return { ...d, title, description, body, ids, headings, sections };
}

function sidebar(pages: Page[], current: string, base: string) {
  return GROUPS.map(
    (g) => `<div class="side-group">
            <h2>${esc(g.title)}</h2>
            ${g.pages
              .filter((p) => pages.some((x) => x.slug === p.slug))
              .map(
                (p) =>
                  `<a href="${base}docs/${p.slug}/"${p.slug === current ? ' aria-current="page"' : ""}>${icon(p.icon)}<span>${esc(p.nav)}</span></a>`,
              )
              .join("\n            ")}
          </div>`,
  ).join("\n          ");
}

function docPage(
  p: Page,
  pages: Page[],
  v: Record<string, string>,
  assets: Assets,
) {
  const base = "../../";
  const i = pages.findIndex((x) => x.slug === p.slug);
  const prev = pages[i - 1];
  const next = pages[i + 1];
  const toc = p.headings.filter((h) => h.level === 2);
  const pager = `<nav class="pager" aria-label="More docs">
            ${prev ? `<a class="pager-prev" href="../${prev.slug}/"><span>${icon("chevron-left")}Previous</span><strong>${esc(prev.nav)}</strong></a>` : "<span></span>"}
            ${next ? `<a class="pager-next" href="../${next.slug}/"><span>Next${icon("chevron-right")}</span><strong>${esc(next.nav)}</strong></a>` : "<span></span>"}
          </nav>`;
  return `${head({
    title: `${p.title} · Legato`,
    description: p.description,
    url: `${SITE}docs/${p.slug}/`,
    base,
    css: assets.css,
    js: assets.js,
  })}
  <body class="docs-body">
    <a class="skip" href="#main">Skip to the page</a>
    ${header(base, "docs")}
    <div class="docs">
      <aside class="side" aria-label="Docs">
        <button class="side-toggle" type="button" data-side-toggle aria-expanded="false">${icon(p.icon)}<span>${esc(p.nav)}</span>${icon("chevron-down", "icon chev")}</button>
        <nav class="side-nav" data-side-nav>
          ${sidebar(pages, p.slug, base)}
        </nav>
      </aside>
      <main id="main" class="doc-main">
        <article class="doc">
          <p class="crumb"><a href="../">Docs</a>${icon("chevron-right")}<span>${esc(p.group)}</span></p>
          <h1>${esc(p.title)}</h1>
${p.body}
          <div class="doc-foot">
            <a href="${BLOB}docs/${p.slug}.md" rel="noopener">${icon("square-pen")}Edit this page on GitHub</a>
            <a href="${GITHUB}/issues/new" rel="noopener">${icon("triangle-alert")}Something wrong? Open an issue</a>
          </div>
          ${pager}
        </article>
      </main>
      ${
        toc.length > 1
          ? `<aside class="toc" aria-label="On this page">
        <h2>On this page</h2>
        <nav data-toc>
          ${toc.map((h) => `<a href="#${h.id}">${esc(h.text)}</a>`).join("\n          ")}
        </nav>
      </aside>`
          : '<div class="toc"></div>'
      }
    </div>
    ${footer(base, v)}
    ${searchDialog}
  </body>
</html>
`;
}

function docsHome(pages: Page[], v: Record<string, string>, assets: Assets) {
  const base = "../";
  const groups = GROUPS.map((g) => {
    const items = g.pages
      .map((gp) => pages.find((p) => p.slug === gp.slug))
      .filter((p): p is Page => !!p)
      .map(
        (p) => `<a class="card doc-card" href="${p.slug}/" data-reveal>
              <span class="card-i">${icon(p.icon)}</span>
              <strong>${esc(p.nav)}</strong>
              <span>${esc(p.description)}</span>
            </a>`,
      )
      .join("\n            ");
    return `<section class="docs-group">
          <h2>${esc(g.title)}</h2>
          <div class="doc-cards">
            ${items}
          </div>
        </section>`;
  }).join("\n        ");
  return `${head({
    title: "Docs · Legato",
    description:
      "How to install Legato on Windows and Android, dictate, take meeting notes, and keep your data where you want it.",
    url: `${SITE}docs/`,
    base,
    css: assets.css,
    js: assets.js,
  })}
  <body>
    <a class="skip" href="#main">Skip to the page</a>
    ${header(base, "docs")}
    <main id="main" class="wrap docs-home">
      <div class="docs-hero" data-reveal>
        <h1>Legato docs</h1>
        <p>How to install it, dictate, take meeting notes, and keep your data where you want it.</p>
        <button class="search-big" type="button" data-search-open>${icon("search")}<span>Search the docs</span><kbd>Ctrl K</kbd></button>
      </div>
      ${groups}
    </main>
    ${footer(base, v)}
    ${searchDialog}
  </body>
</html>
`;
}

function notFound(v: Record<string, string>, assets: Assets) {
  // GitHub Pages serves this at any depth, so it links from the site root.
  const base = "/";
  return `${head({ title: "Page not found · Legato", description: "This page isn't here.", url: SITE, base, css: assets.css, js: assets.js })}
  <body>
    ${header(base, "home")}
    <main id="main" class="wrap missing">
      <h1>This page isn't here</h1>
      <p>It may have moved when the docs were reorganised.</p>
      <p class="missing-links"><a class="btn" href="/docs/">${icon("book-a")}Browse the docs</a><a class="btn btn-quiet" href="/">Home</a></p>
    </main>
    ${footer(base, v)}
    ${searchDialog}
  </body>
</html>
`;
}

// Fails the build on a link to a docs page or heading that doesn't exist.
function checkLinks(pages: Page[], index: string) {
  const bySlug = new Map(pages.map((p) => [p.slug, p]));
  const bad: string[] = [];
  const check = (href: string, from: string, own: Page | null) => {
    const m = href.match(
      /^(?:\.\.\/([a-z0-9-]+)\/|docs\/([a-z0-9-]+)\/)?(?:#(.+))?$/,
    );
    if (!m || (!m[1] && !m[2] && !m[3])) return;
    const target = m[1] || m[2] ? bySlug.get((m[1] || m[2])!) : own;
    if (!target) return void bad.push(`${from}: ${href} (no such page)`);
    if (m[3] && target !== null && !target.ids.has(m[3]))
      bad.push(`${from}: ${href} (no such heading)`);
  };
  for (const p of pages)
    for (const [, h] of p.body.matchAll(/href="([^"]+)"/g)) check(h, p.slug, p);
  for (const [, h] of index.matchAll(/href="(docs\/[^"]+)"/g))
    check(h, "index", null);
  if (bad.length) throw new Error(`broken links:\n  ${bad.join("\n  ")}`);
}

// ---------- build ----------

type Assets = { css: string; js: string };

const rel = await latestRelease();
const values = downloads(rel);
const missing = DOCS.filter(
  (d) => !existsSync(join(root, "docs", `${d.slug}.md`)),
);
if (missing.length)
  throw new Error(
    `docs listed but missing: ${missing.map((d) => d.slug).join(", ")}`,
  );
const pages = DOCS.map(renderDoc);

rmSync(out, { recursive: true, force: true });
mkdirSync(join(out, "fonts"), { recursive: true });
const copies: [string, string][] = [
  ["site/site.css", "site.css"],
  ["site/site.js", "site.js"],
  ["docs/hero-light.webp", "hero-light.webp"],
  ["docs/hero-dark.webp", "hero-dark.webp"],
  ["docs/how-it-works-anim.svg", "how-it-works-anim.svg"],
  ["src/assets/legato-icon.svg", "legato-icon.svg"],
  ["src/assets/fonts/figtree-latin.woff2", "fonts/figtree-latin.woff2"],
  ["src/assets/fonts/Figtree-OFL.txt", "fonts/Figtree-OFL.txt"],
];
for (const [from, to] of copies) {
  if (!existsSync(join(root, from))) throw new Error(`missing ${from}`);
  cpSync(join(root, from), join(out, to));
}
const assets: Assets = {
  css: `site.css?v=${hashOf(join(out, "site.css"))}`,
  js: `site.js?v=${hashOf(join(out, "site.js"))}`,
};

const index = fill(
  readFileSync(join(import.meta.dir, "index.html"), "utf8"),
  values,
)
  .replace(
    /<!--head-->/,
    head({
      title: "Legato: dictation and meeting notes for Windows",
      description:
        "Dictation and meeting notes for Windows, transcribed on your PC, with a phone app that dictates in any app. Free and open source.",
      url: SITE,
      base: "",
      css: assets.css,
      js: assets.js,
    }).replace(/^<!doctype html>\n<html lang="en">\n/, ""),
  )
  .replace("<!--header-->", header("", "home"))
  .replace("<!--footer-->", footer("", values))
  .replace("<!--search-->", searchDialog);
checkLinks(pages, index);

writeFileSync(join(out, "index.html"), index);
writeFileSync(join(out, "404.html"), notFound(values, assets));
mkdirSync(join(out, "docs"), { recursive: true });
writeFileSync(join(out, "docs", "index.html"), docsHome(pages, values, assets));
for (const p of pages) {
  const file = join(out, "docs", p.slug, "index.html");
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, docPage(p, pages, values, assets));
}
// One entry per section, for the search box.
writeFileSync(
  join(out, "search.json"),
  JSON.stringify(
    pages.flatMap((p) =>
      p.sections.map((s) => ({
        u: `docs/${p.slug}/${s.id ? `#${s.id}` : ""}`,
        p: p.nav,
        h: s.heading,
        t: s.text,
      })),
    ),
  ),
);
const urls = [
  SITE,
  `${SITE}docs/`,
  ...pages.map((p) => `${SITE}docs/${p.slug}/`),
];
writeFileSync(
  join(out, "sitemap.xml"),
  `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls.map((u) => `  <url><loc>${u}</loc></url>`).join("\n")}\n</urlset>\n`,
);
writeFileSync(
  join(out, "robots.txt"),
  `User-agent: *\nAllow: /\nSitemap: ${SITE}sitemap.xml\n`,
);
console.log(
  `built ${out}: the home page, a docs home and ${pages.length} docs, release ${values.version}`,
);
