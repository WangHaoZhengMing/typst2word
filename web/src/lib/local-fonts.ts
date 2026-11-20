//! On-demand Local Font Access API (`window.queryLocalFonts()`) helper for Chromium browsers.
//!
//! Queries only the lightweight font catalog upon user gesture, then reads `face.blob()` bytes
//! strictly for font families actually referenced in the active `.typ` sources.

export interface LocalFontData {
  readonly family: string;
  readonly fullName: string;
  readonly postscriptName: string;
  readonly style: string;
  blob(): Promise<Blob>;
}

const CJK_REGEX = /[\u3040-\u30ff\u3400-\u9fff\uf900-\ufaff]/;

const FAMILY_ALIASES: Record<string, string[]> = {
  宋体: ['SimSun', 'Songti SC', 'STSong', 'NSimSun'],
  simsun: ['SimSun', '宋体', 'Songti SC'],
  'songti sc': ['Songti SC', 'STSong', 'SimSun'],
  黑体: ['SimHei', 'Heiti SC', 'STHeiti', 'PingFang SC'],
  simhei: ['SimHei', '黑体', 'Heiti SC'],
  'heiti sc': ['Heiti SC', 'STHeiti', 'SimHei'],
  楷体: ['KaiTi', 'Kaiti SC', 'STKaiti'],
  kaiti: ['KaiTi', 'Kaiti SC', 'STKaiti'],
  仿宋: ['FangSong', 'STFangsong'],
  微软雅黑: ['Microsoft YaHei', 'PingFang SC'],
  'microsoft yahei': ['Microsoft YaHei', '微软雅黑'],
  苹方: ['PingFang SC'],
  'pingfang sc': ['PingFang SC'],
};

const DEFAULT_LOCAL_CJK_FALLBACKS = [
  'Songti SC',
  'SimSun',
  'PingFang SC',
  'Microsoft YaHei',
  'Noto Serif CJK SC',
  'Noto Sans CJK SC',
  'Source Han Serif SC',
  'STSong',
];

const CORE_STYLES = new Set([
  'regular',
  'normal',
  'book',
  'roman',
  'bold',
  'semibold',
  'medium',
  'italic',
  'oblique',
  'bold italic',
  'bold oblique',
]);

export function isLocalFontApiSupported(): boolean {
  return (
    typeof window !== 'undefined' &&
    typeof (window as unknown as { queryLocalFonts?: unknown }).queryLocalFonts === 'function'
  );
}

export async function queryLocalFontCatalog(): Promise<LocalFontData[]> {
  if (!isLocalFontApiSupported()) {
    throw new Error('当前浏览器不支持 Local Font Access API');
  }
  const fn = (window as unknown as { queryLocalFonts: () => Promise<LocalFontData[]> })
    .queryLocalFonts;
  return await fn();
}

/** Extract font family names referenced via `font: "..."` or `font: ("A", "B")` in Typst sources. */
export function extractRequestedFontFamilies(typSources: string[]): string[] {
  const found = new Set<string>();
  const singleRe = /\bfont\s*:\s*"([^"]+)"/g;
  const tupleRe = /\bfont\s*:\s*\(([^)]*)\)/g;
  const quotedRe = /"([^"]+)"/g;

  for (const src of typSources) {
    let m: RegExpExecArray | null;
    singleRe.lastIndex = 0;
    while ((m = singleRe.exec(src)) !== null) {
      const name = m[1].trim();
      if (name) found.add(name);
    }
    tupleRe.lastIndex = 0;
    while ((m = tupleRe.exec(src)) !== null) {
      const inner = m[1];
      let qm: RegExpExecArray | null;
      quotedRe.lastIndex = 0;
      while ((qm = quotedRe.exec(inner)) !== null) {
        const name = qm[1].trim();
        if (name) found.add(name);
      }
    }
  }
  return Array.from(found);
}

function isCoreStyle(style: string): boolean {
  const s = style.trim().toLowerCase();
  if (!s) return true;
  return CORE_STYLES.has(s);
}

function pickFacesForFamily(faces: LocalFontData[], maxFaces = 4): LocalFontData[] {
  const core = faces.filter((f) => isCoreStyle(f.style));
  const pool = core.length > 0 ? core : faces;
  // Prioritize Regular, Bold, Italic, Bold Italic
  const rank = (style: string) => {
    const s = style.trim().toLowerCase();
    if (s === 'regular' || s === 'normal' || s === 'book' || s === 'roman') return 0;
    if (s === 'bold') return 1;
    if (s === 'italic' || s === 'oblique') return 2;
    if (s === 'bold italic' || s === 'bold oblique') return 3;
    if (s === 'semibold' || s === 'medium') return 4;
    return 5;
  };
  return [...pool].sort((a, b) => rank(a.style) - rank(b.style)).slice(0, maxFaces);
}

export interface LoadedLocalFontBlob {
  name: string;
  family: string;
  buffer: ArrayBuffer;
  isSystemInstalled: boolean;
}

/**
 * Match requested Typst font families against the authorized local font catalog and read
 * binary font blobs ONLY for newly needed faces (never loading all system fonts).
 */
export async function loadMatchingLocalFonts(
  catalog: LocalFontData[],
  typSources: string[],
  loadedFaceKeys: Set<string>,
): Promise<{
  entries: LoadedLocalFontBlob[];
  matchedFamilies: string[];
}> {
  if (catalog.length === 0) {
    return { entries: [], matchedFamilies: [] };
  }

  // Group catalog by lowercased family name
  const byFamily = new Map<string, LocalFontData[]>();
  for (const face of catalog) {
    const key = face.family.trim().toLowerCase();
    const list = byFamily.get(key);
    if (list) list.push(face);
    else byFamily.set(key, [face]);
  }

  const requested = extractRequestedFontFamilies(typSources);
  const targetFaces: LocalFontData[] = [];
  const matchedFamilies = new Set<string>();

  const trySelectFamily = (rawName: string, maxFaces = 4): boolean => {
    const lower = rawName.trim().toLowerCase();
    const candidates = [rawName, ...(FAMILY_ALIASES[lower] ?? [])];
    for (const cand of candidates) {
      const list = byFamily.get(cand.trim().toLowerCase());
      if (list && list.length > 0) {
        const picked = pickFacesForFamily(list, maxFaces);
        for (const f of picked) {
          const faceKey = f.postscriptName || `${f.family}::${f.style}`;
          if (!loadedFaceKeys.has(faceKey)) {
            loadedFaceKeys.add(faceKey);
            targetFaces.push(f);
          }
        }
        matchedFamilies.add(list[0].family);
        return true;
      }
    }
    return false;
  };

  for (const fam of requested) {
    trySelectFamily(fam, 4);
  }

  // If document contains CJK text and no CJK family was matched yet, pick one native local CJK family.
  const hasCjkText = typSources.some((s) => CJK_REGEX.test(s));
  const hasMatchedCjk = Array.from(matchedFamilies).some((f) => {
    const l = f.toLowerCase();
    return (
      l.includes('song') ||
      l.includes('pingfang') ||
      l.includes('heiti') ||
      l.includes('simsun') ||
      l.includes('simhei') ||
      l.includes('yahei') ||
      l.includes('kaiti') ||
      l.includes('fangsong') ||
      l.includes('cjk') ||
      l.includes('han') ||
      l.includes('sc')
    );
  });
  if (hasCjkText && !hasMatchedCjk) {
    for (const fallback of DEFAULT_LOCAL_CJK_FALLBACKS) {
      if (trySelectFamily(fallback, 2)) {
        break;
      }
    }
  }

  const entries: LoadedLocalFontBlob[] = [];
  for (const face of targetFaces) {
    try {
      const blob = await face.blob();
      const buffer = await blob.arrayBuffer();
      entries.push({
        name: face.postscriptName || `${face.family}-${face.style}`,
        family: face.family,
        buffer,
        isSystemInstalled: true,
      });
    } catch {
      // Skip individual unreadable system faces cleanly
    }
  }

  return {
    entries,
    matchedFamilies: Array.from(matchedFamilies),
  };
}
