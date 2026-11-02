// Builds the project page and the docs into _site (or --out <dir>).
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

const SITE = "https://legato.abhashchakraborty.tech/";
const REPO = "Abhash-Chakraborty/legato";
const LATEST = `https://github.com/${REPO}/releases/latest`;
const BLOB = `https://github.com/${REPO}/blob/main/`;

// Sidebar order; each is docs/<slug>.md.
const DOCS: { slug: string; nav: string }[] = [
  { slug: "install", nav: "Install on Windows" },
  { slug: "android", nav: "Legato for Android" },
  { slug: "troubleshooting", nav: "Troubleshooting" },
  { slug: "verify", nav: "Verify a download" },
  { slug: "cli", nav: "Command line" },
  { slug: "phone-sync", nav: "Phone sync protocol" },
];

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

function mb(bytes: number) {
  return ` · ${(bytes / 1024 / 1024).toFixed(bytes < 10 * 1024 * 1024 ? 1 : 0)} MB`;
}

function downloads(rel: Release | null): Record<string, string> {
  const files: Record<string, RegExp> = {
    x64: /_x64-setup\.exe$/,
    arm64: /_arm64-setup\.exe$/,
    msi: /_x64_en-US\.msi$/,
    apk: /^Legato-android-.+\.apk$/,
  };
  const v: Record<string, string> = {
    site: SITE,
    release: rel?.html_url ?? LATEST,
    version: rel ? rel.tag_name.replace(/^v/, "") : "latest",
    released: "",
  };
  if (rel) {
    const day = new Date(rel.published_at).toLocaleDateString("en-GB", {
      day: "numeric",
      month: "long",
      year: "numeric",
      timeZone: "UTC",
    });
    v.released = `Version ${esc(v.version)}, released ${day}. `;
  }
  for (const [key, re] of Object.entries(files)) {
    const a = rel?.assets.find((x) => re.test(x.name));
    if (rel && !a)
      throw new Error(`the latest release has no file matching ${re}`);
    v[key] = a ? a.browser_download_url : LATEST;
    v[`${key}-name`] = a ? esc(a.name) : "On the latest release";
    v[`${key}-size`] = a ? mb(a.size) : "";
  }
  return v;
}

function fill(html: string, v: Record<string, string>) {
  return html.replace(/\{\{([a-z0-9-]+)\}\}/g, (m, k) => {
    if (!(k in v)) throw new Error(`no value for ${m}`);
    return v[k];
  });
}

function header(base: string) {
  return `<header>
        <a class="brand" href="${base}"><img src="${base}legato-icon.svg" alt="" />Legato</a>
        <nav>
          <a href="${base}#features" class="hide-small">Features</a>
          <a href="${base}#phone" class="hide-small">Phone</a>
          <a href="${base}#download">Download</a>
          <a href="${base}docs/install/">Docs</a>
          <a href="https://github.com/${REPO}" class="hide-small">GitHub</a>
        </nav>
      </header>`;
}

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

function plain(html: string) {
  return html
    .replace(/<[^>]+>/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

type Page = {
  slug: string;
  title: string;
  description: string;
  body: string;
  ids: Set<string>;
};

function renderDoc(slug: string): Page {
  const md = readFileSync(join(root, "docs", `${slug}.md`), "utf8");
  let body = Bun.markdown.html(md, { headings: { ids: true } });
  const title = plain(body.match(/<h1[^>]*>(.*?)<\/h1>/s)?.[1] ?? slug);
  const first = plain(body.match(/<p>(.*?)<\/p>/s)?.[1] ?? "");
  const description =
    first.length > 160
      ? `${first.slice(0, first.lastIndexOf(" ", 157))}…`
      : first;
  body = body
    .replace(/<table>/g, '<div class="table"><table>')
    .replace(/<\/table>/g, "</table></div>")
    .replace(
      /href="([^"]+)"/g,
      (_, h) => `href="${esc(link(h.replace(/&amp;/g, "&"), slug))}"`,
    );
  const ids = new Set([...body.matchAll(/ id="([^"]+)"/g)].map((m) => m[1]));
  return { slug, title, description, body, ids };
}

function docPage(p: Page) {
  const nav = DOCS.map(
    (d) =>
      `<a href="../${d.slug}/"${d.slug === p.slug ? ' aria-current="page"' : ""}>${esc(d.nav)}</a>`,
  ).join("\n          ");
  const url = `${SITE}docs/${p.slug}/`;
  return `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>${esc(p.title)} · Legato</title>
    <meta name="description" content="${esc(p.description)}" />
    <link rel="canonical" href="${url}" />
    <meta property="og:title" content="${esc(p.title)} · Legato" />
    <meta property="og:description" content="${esc(p.description)}" />
    <meta property="og:url" content="${url}" />
    <meta property="og:image" content="${SITE}hero-light.webp" />
    <link rel="icon" href="../../legato-icon.svg" type="image/svg+xml" />
    <link rel="stylesheet" href="../../site.css" />
  </head>
  <body>
    <div class="wrap">
      ${header("../../")}
      <div class="docs">
        <nav class="docnav" aria-label="Docs">
          ${nav}
        </nav>
        <article class="doc">
${p.body}
          <p class="doc-foot">
            <a href="${BLOB}docs/${p.slug}.md">Edit this page on GitHub</a>
          </p>
        </article>
      </div>
    </div>
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
    if (m[3] && own !== null && !target.ids.has(m[3]))
      bad.push(`${from}: ${href} (no such heading)`);
  };
  for (const p of pages)
    for (const [, h] of p.body.matchAll(/href="([^"]+)"/g)) check(h, p.slug, p);
  for (const [, h] of index.matchAll(/href="(docs\/[^"]+)"/g))
    check(h, "index", null);
  if (bad.length) throw new Error(`broken links:\n  ${bad.join("\n  ")}`);
}

const rel = await latestRelease();
const values = downloads(rel);
const index = fill(
  readFileSync(join(import.meta.dir, "index.html"), "utf8"),
  values,
);
const pages = DOCS.map((d) => renderDoc(d.slug));
checkLinks(pages, index);

rmSync(out, { recursive: true, force: true });
mkdirSync(join(out, "fonts"), { recursive: true });
writeFileSync(join(out, "index.html"), index);
for (const p of pages) {
  const file = join(out, "docs", p.slug, "index.html");
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, docPage(p));
}
const copies: [string, string][] = [
  ["site/site.css", "site.css"],
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
const urls = [SITE, ...pages.map((p) => `${SITE}docs/${p.slug}/`)];
writeFileSync(
  join(out, "sitemap.xml"),
  `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls.map((u) => `  <url><loc>${u}</loc></url>`).join("\n")}\n</urlset>\n`,
);
writeFileSync(
  join(out, "robots.txt"),
  `User-agent: *\nAllow: /\nSitemap: ${SITE}sitemap.xml\n`,
);
console.log(
  `built ${out}: the page and ${pages.length} docs, release ${values.version}`,
);
