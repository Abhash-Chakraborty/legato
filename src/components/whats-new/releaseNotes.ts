export interface ReleaseNote {
  version: string;
  markdown: string;
}

interface FindReleaseNoteOptions {
  currentVersion: string;
  lastSeenVersion: string;
}

// Each version's notes load when they're shown: the list of versions comes
// from the file names alone.
const loaders = import.meta.glob<string>("../../content/release-notes/*.md", {
  import: "default",
  query: "?raw",
});

const parseVersion = (version: string): [number, number, number] | null => {
  const normalized = version.trim().replace(/^v/i, "");
  const match = normalized.match(/^(\d+)\.(\d+)\.(\d+)/);
  if (!match) return null;

  return [Number(match[1]), Number(match[2]), Number(match[3])];
};

const loadersByVersion = new Map<string, () => Promise<string>>();

for (const [path, load] of Object.entries(loaders)) {
  const match = path.match(/release-notes\/([^/]+)\.md$/);
  if (!match || !parseVersion(match[1])) continue;
  loadersByVersion.set(match[1], load);
}

const read = async (
  version: string | undefined,
): Promise<ReleaseNote | null> => {
  const load = version ? loadersByVersion.get(version) : undefined;
  if (!version || !load) return null;
  return { version, markdown: await load() };
};

const compareVersions = (a: string, b: string) => {
  const parsedA = parseVersion(a);
  const parsedB = parseVersion(b);

  if (!parsedA || !parsedB) return 0;

  for (let index = 0; index < parsedA.length; index += 1) {
    if (parsedA[index] !== parsedB[index]) {
      return parsedA[index] - parsedB[index];
    }
  }

  return 0;
};

/** The newest notes at or below this version that haven't been seen. */
export const findReleaseNoteToShow = ({
  currentVersion,
  lastSeenVersion,
}: FindReleaseNoteOptions): Promise<ReleaseNote | null> => {
  if (!parseVersion(currentVersion)) return Promise.resolve(null);

  const hasValidLastSeenVersion = parseVersion(lastSeenVersion) !== null;
  const candidate = Array.from(loadersByVersion.keys())
    .filter((version) => compareVersions(version, currentVersion) <= 0)
    .filter(
      (version) =>
        !hasValidLastSeenVersion ||
        compareVersions(version, lastSeenVersion) > 0,
    )
    .sort((a, b) => compareVersions(b, a))[0];

  return read(candidate);
};

/** The newest notes there are. */
export const findLatestReleaseNote = (): Promise<ReleaseNote | null> =>
  read(
    Array.from(loadersByVersion.keys()).sort((a, b) =>
      compareVersions(b, a),
    )[0],
  );
