/// <reference lib="webworker" />

import initWasm, { WasmWorkspace } from './wasm-pkg/typst2word_wasm.js';
import wasmUrl from './wasm-pkg/typst2word_wasm_bg.wasm?url';
import {
  getCachedFont,
  getCachedPackage,
  setCachedFont,
  setCachedPackage,
} from './idb-cache';
import type {
  ConversionReport,
  FontFamilyDto,
  MissingPackageDto,
  OpenXmlPackageDto,
  PreviewResponseDto,
} from './types';

declare const self: DedicatedWorkerGlobalScope;

const CLOUD_CJK_FONTS: Array<{
  cacheKey: string;
  label: string;
  urls: string[];
}> = [
  {
    cacheKey: 'noto-serif-sc-400-v36',
    label: 'Noto Serif SC Regular',
    urls: [
      'https://fonts.gstatic.com/s/notoserifsc/v36/H4cyBXePl9DZ0Xe7gG9cyOj7uK2-n-D2rd4FY7SCqyWv.ttf',
      'https://cdn.jsdelivr.net/gh/notofonts/noto-cjk@main/Sans/OTF/SimplifiedChinese/NotoSansCJKsc-Regular.otf',
    ],
  },
  {
    cacheKey: 'noto-serif-sc-700-v36',
    label: 'Noto Serif SC Bold',
    urls: [
      'https://fonts.gstatic.com/s/notoserifsc/v36/H4cyBXePl9DZ0Xe7gG9cyOj7uK2-n-D2rd4FY7RlrCWv.ttf',
    ],
  },
];

const CJK_REGEX = /[\u3040-\u30ff\u3400-\u9fff\uf900-\ufaff]/;

let ws: WasmWorkspace | null = null;
let initPromise: Promise<WasmWorkspace> | null = null;
let cloudCjkLoaded = false;
let cloudCjkLoadingPromise: Promise<number> | null = null;

function postEvent(payload: Record<string, unknown>, transfer?: Transferable[]) {
  if (transfer && transfer.length > 0) {
    self.postMessage(payload, transfer);
  } else {
    self.postMessage(payload);
  }
}

function listFonts(workspace: WasmWorkspace): FontFamilyDto[] {
  try {
    return JSON.parse(workspace.list_font_families_json()) as FontFamilyDto[];
  } catch {
    return [];
  }
}

function listPackages(workspace: WasmWorkspace): string[] {
  try {
    return JSON.parse(workspace.loaded_packages_json()) as string[];
  } catch {
    return [];
  }
}

function hasAnyCjkFont(workspace: WasmWorkspace): boolean {
  if (cloudCjkLoaded) return true;
  const cjkHints = [
    'noto serif sc',
    'noto sans sc',
    'noto sans cjk',
    'noto serif cjk',
    'source han',
    'songti',
    'pingfang',
    'heiti',
    'kaiti',
    'fangsong',
    'simsun',
    'simhei',
    'microsoft yahei',
    'dengxian',
    'stsong',
    'fandol',
    'wenkai',
  ];
  return listFonts(workspace).some((f) => {
    const lower = f.family.toLowerCase();
    return cjkHints.some((h) => lower.includes(h));
  });
}

async function fetchFirstAvailable(urls: string[]): Promise<Uint8Array> {
  let lastErr: unknown = null;
  for (const url of urls) {
    try {
      const resp = await fetch(url, { mode: 'cors' });
      if (!resp.ok) {
        throw new Error(`HTTP ${resp.status} for ${url}`);
      }
      const buf = await resp.arrayBuffer();
      return new Uint8Array(buf);
    } catch (err) {
      lastErr = err;
    }
  }
  throw lastErr instanceof Error ? lastErr : new Error('Failed to fetch cloud font');
}

async function ensureCloudCjkFonts(
  workspace: WasmWorkspace,
  notifyRecompile: boolean,
): Promise<number> {
  if (cloudCjkLoaded) return 0;
  if (cloudCjkLoadingPromise) return cloudCjkLoadingPromise;

  cloudCjkLoadingPromise = (async () => {
    let totalFaces = 0;
    try {
      for (const item of CLOUD_CJK_FONTS) {
        let bytes = await getCachedFont(item.cacheKey);
        if (!bytes) {
          postEvent({
            type: 'status',
            phase: 'loading-font',
            message: `正在加载中文排版字体 ${item.label}…`,
          });
          bytes = await fetchFirstAvailable(item.urls);
          await setCachedFont(item.cacheKey, bytes);
        }
        totalFaces += workspace.add_font_bytes(bytes, false);
      }
      if (totalFaces > 0) {
        cloudCjkLoaded = true;
        postEvent({
          type: 'fonts-updated',
          fonts: listFonts(workspace),
          autoRecompile: notifyRecompile,
        });
      }
    } catch (err) {
      console.warn('[typst2word-worker] Cloud CJK font load warning:', err);
    } finally {
      cloudCjkLoadingPromise = null;
    }
    return totalFaces;
  })();

  return cloudCjkLoadingPromise;
}

async function preloadCachedCjkFontsIfPresent(workspace: WasmWorkspace): Promise<void> {
  try {
    let added = 0;
    let allCached = true;
    for (const item of CLOUD_CJK_FONTS) {
      const cached = await getCachedFont(item.cacheKey);
      if (cached) {
        added += workspace.add_font_bytes(cached, false);
      } else {
        allCached = false;
      }
    }
    if (added > 0 && allCached) {
      cloudCjkLoaded = true;
    }
  } catch {
    // Ignore IndexedDB errors in private browsing modes.
  }
}

async function fetchWasmWithProgress(url: string): Promise<Uint8Array> {
  const resp = await fetch(url);
  if (!resp.ok) {
    throw new Error(`HTTP ${resp.status} while fetching WASM binary`);
  }
  const totalHeader = Number(resp.headers.get('content-length') || 0);
  if (!resp.body) {
    const buf = new Uint8Array(await resp.arrayBuffer());
    return buf;
  }

  const reader = resp.body.getReader();
  const chunks: Uint8Array[] = [];
  let loaded = 0;
  let lastEmit = 0;

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    if (value) {
      chunks.push(value);
      loaded += value.byteLength;
      const now = performance.now();
      if (now - lastEmit >= 70) {
        lastEmit = now;
        const mbLoaded = (loaded / (1024 * 1024)).toFixed(1);
        if (totalHeader > 0) {
          const pct = Math.min(92, Math.round((loaded / totalHeader) * 92));
          const mbTotal = (totalHeader / (1024 * 1024)).toFixed(1);
          postEvent({
            type: 'status',
            phase: 'booting',
            progress: pct,
            message: `正在下载引擎 ${pct}% (${mbLoaded} / ${mbTotal} MB)`,
          });
        } else {
          const pseudoPct = Math.min(88, Math.round((1 - Math.exp(-loaded / (8 * 1024 * 1024))) * 90));
          postEvent({
            type: 'status',
            phase: 'booting',
            progress: pseudoPct,
            message: `正在下载引擎 ${mbLoaded} MB…`,
          });
        }
      }
    }
  }

  const merged = new Uint8Array(loaded);
  let offset = 0;
  for (const chunk of chunks) {
    merged.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return merged;
}

async function ensureWorkspace(): Promise<WasmWorkspace> {
  if (ws) return ws;
  if (!initPromise) {
    initPromise = (async () => {
      postEvent({
        type: 'status',
        phase: 'booting',
        progress: 2,
        message: '正在连接并下载 WASM 引擎…',
      });
      const wasmBytes = await fetchWasmWithProgress(wasmUrl);
      postEvent({
        type: 'status',
        phase: 'booting',
        progress: 95,
        message: '正在编译并初始化 WASM 模块…',
      });
      await initWasm({ module_or_path: wasmBytes });
      const instance = new WasmWorkspace();
      await preloadCachedCjkFontsIfPresent(instance);
      ws = instance;
      postEvent({
        type: 'booted',
        fonts: listFonts(instance),
        packages: listPackages(instance),
      });
      return instance;
    })();
  }
  return initPromise;
}

async function resolveMissingPackages(
  workspace: WasmWorkspace,
  missing: MissingPackageDto[],
): Promise<void> {
  await Promise.all(
    missing.map(async (pkg) => {
      if (workspace.has_package(pkg.namespace, pkg.name, pkg.version)) {
        return;
      }
      let tarGz = await getCachedPackage(pkg.spec);
      if (!tarGz) {
        postEvent({
          type: 'status',
          phase: 'fetching-package',
          message: `正在拉取 ${pkg.spec}…`,
        });
        const resp = await fetch(pkg.download_url, { mode: 'cors' });
        if (!resp.ok) {
          throw new Error(`无法拉取包 ${pkg.spec} (HTTP ${resp.status})`);
        }
        tarGz = new Uint8Array(await resp.arrayBuffer());
        await setCachedPackage(pkg.spec, tarGz);
      }
      workspace.register_package_tar_gz(pkg.namespace, pkg.name, pkg.version, tarGz);
    }),
  );
  postEvent({
    type: 'packages-updated',
    packages: listPackages(workspace),
  });
}

async function compilePreviewWithPackages(
  workspace: WasmWorkspace,
): Promise<{ dto: PreviewResponseDto; elapsedMs: number }> {
  const t0 = performance.now();
  let dto: PreviewResponseDto = JSON.parse(workspace.compile_preview_json());
  let hops = 0;

  while (dto.status === 'missing_packages' && dto.missing_packages.length > 0 && hops < 10) {
    hops += 1;
    await resolveMissingPackages(workspace, dto.missing_packages);
    dto = JSON.parse(workspace.compile_preview_json());
  }

  const elapsedMs = Math.round(performance.now() - t0);
  return { dto, elapsedMs };
}

function syncWorkspaceState(
  workspace: WasmWorkspace,
  payload: {
    mainPath: string;
    files: Array<{ path: string; bytes: Uint8Array; text: string | null }>;
    inputs: Record<string, string>;
  },
): boolean {
  workspace.clear_files();
  let hasCjk = false;

  for (const file of payload.files) {
    if (file.text !== null && CJK_REGEX.test(file.text)) {
      hasCjk = true;
    }
    if (file.path === payload.mainPath && file.text !== null) {
      workspace.set_main_source(file.path, file.text);
    } else {
      workspace.put_file(file.path, file.bytes);
    }
  }
  workspace.set_main_path(payload.mainPath);
  workspace.set_inputs_json(JSON.stringify(payload.inputs || {}));

  return hasCjk;
}

self.onmessage = async (event: MessageEvent) => {
  const msg = event.data;
  if (!msg || typeof msg.id !== 'number') return;
  const { id, action, payload } = msg;

  try {
    const workspace = await ensureWorkspace();

    switch (action) {
      case 'boot': {
        postEvent({
          id,
          ok: true,
          result: {
            fonts: listFonts(workspace),
            packages: listPackages(workspace),
          },
        });
        break;
      }

      case 'sync-and-preview': {
        const hasCjk = syncWorkspaceState(workspace, payload);
        postEvent({
          type: 'status',
          phase: 'compiling',
          message: '正在编译排版与布局度量…',
        });

        const { dto, elapsedMs } = await compilePreviewWithPackages(workspace);

        postEvent({
          id,
          ok: true,
          result: {
            preview: dto,
            elapsedMs,
            fonts: listFonts(workspace),
            packages: listPackages(workspace),
          },
        });

        // If CJK characters are present and no CJK font is available yet, load cloud Noto Serif SC
        // asynchronously in the background and trigger a crisp re-render upon completion!
        if (hasCjk && !hasAnyCjkFont(workspace)) {
          void ensureCloudCjkFonts(workspace, true);
        }
        break;
      }

      case 'compile-only-preview': {
        postEvent({
          type: 'status',
          phase: 'compiling',
          message: '正在刷新排版预览…',
        });
        const { dto, elapsedMs } = await compilePreviewWithPackages(workspace);
        postEvent({
          id,
          ok: true,
          result: {
            preview: dto,
            elapsedMs,
            fonts: listFonts(workspace),
            packages: listPackages(workspace),
          },
        });
        break;
      }

      case 'export-docx': {
        if (payload?.embedMode && typeof (workspace as any).set_embed_fonts_mode === 'function') {
          (workspace as any).set_embed_fonts_mode(String(payload.embedMode));
        }
        if (payload?.workspace) {
          const hasCjk = syncWorkspaceState(workspace, payload.workspace);
          if (hasCjk && !hasAnyCjkFont(workspace)) {
            await ensureCloudCjkFonts(workspace, false);
          }
        }
        postEvent({
          type: 'status',
          phase: 'exporting',
          message: '正在生成原生 OpenXML (.docx)…',
        });

        const t0 = performance.now();
        let hops = 0;
        while (hops < 10) {
          const out = workspace.compile_and_convert(false);
          try {
            if (out.status === 'missing_packages') {
              const missing = JSON.parse(out.missing_packages_json()) as MissingPackageDto[];
              if (missing.length > 0) {
                hops += 1;
                await resolveMissingPackages(workspace, missing);
                continue;
              }
            }

            if (out.status !== 'ok') {
              throw new Error(out.error || '导出 .docx 失败');
            }

            const bytes = out.docx_bytes();
            const report = JSON.parse(out.report_json) as ConversionReport;
            const elapsedMs = Math.round(performance.now() - t0);
            const buffer = bytes.buffer.slice(
              bytes.byteOffset,
              bytes.byteOffset + bytes.byteLength,
            );

            postEvent(
              {
                id,
                ok: true,
                result: {
                  docxBuffer: buffer,
                  report,
                  elapsedMs,
                  fonts: listFonts(workspace),
                  packages: listPackages(workspace),
                },
              },
              [buffer],
            );
            break;
          } finally {
            out.free();
          }
        }
        break;
      }

      case 'inspect-openxml': {
        if (payload?.embedMode && typeof (workspace as any).set_embed_fonts_mode === 'function') {
          (workspace as any).set_embed_fonts_mode(String(payload.embedMode));
        }
        if (payload?.workspace) {
          syncWorkspaceState(workspace, payload.workspace);
        }
        // Make sure any missing @preview packages are resolved first.
        await compilePreviewWithPackages(workspace);
        const pkg = JSON.parse(workspace.export_openxml_json()) as OpenXmlPackageDto;
        postEvent({ id, ok: true, result: pkg });
        break;
      }

      case 'inspect-ir': {
        if (payload?.workspace) {
          syncWorkspaceState(workspace, payload.workspace);
        }
        await compilePreviewWithPackages(workspace);
        const irJson = workspace.export_ir_json();
        postEvent({ id, ok: true, result: { irJson } });
        break;
      }

      case 'add-fonts': {
        const entries = payload.fonts as Array<{
          name: string;
          buffer: ArrayBuffer;
          isSystemInstalled: boolean;
        }>;
        let addedFaces = 0;
        for (const f of entries) {
          addedFaces += workspace.add_font_bytes(new Uint8Array(f.buffer), f.isSystemInstalled);
        }
        const fonts = listFonts(workspace);
        postEvent({
          id,
          ok: true,
          result: { addedFaces, fonts },
        });
        break;
      }

      case 'load-cloud-cjk': {
        const addedFaces = await ensureCloudCjkFonts(workspace, false);
        postEvent({
          id,
          ok: true,
          result: { addedFaces, fonts: listFonts(workspace) },
        });
        break;
      }

      default:
        throw new Error(`Unknown worker action: ${String(action)}`);
    }
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    postEvent({ id, ok: false, error: message });
  }
};
