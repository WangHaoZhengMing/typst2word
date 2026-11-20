<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import JSZip from 'jszip';
  import { TypstWordWorkerClient } from './lib/worker-client';
  import { STUDIO_PRESETS } from './lib/presets';
  import {
    isLocalFontApiSupported,
    loadMatchingLocalFonts,
    queryLocalFontCatalog,
    type LocalFontData,
  } from './lib/local-fonts';
  import XRayInspector from './lib/components/XRayInspector.svelte';
  import type {
    ConversionReport,
    EnginePhase,
    FontEmbedMode,
    FontFamilyDto,
    OpenXmlPackageDto,
    VirtualFile,
  } from './lib/types';

  const encoder = new TextEncoder();
  const decoder = new TextDecoder('utf-8');

  const TEXT_EXTS = new Set([
    'typ',
    'bib',
    'yml',
    'yaml',
    'toml',
    'json',
    'csv',
    'txt',
    'md',
    'svg',
    'xml',
    'csl',
  ]);
  const FONT_EXTS = new Set(['ttf', 'otf', 'ttc', 'otc']);

  type InputMode = 'upload' | 'paste';

  interface ConversionResult {
    docxBytes: Uint8Array;
    filename: string;
    report: ConversionReport;
    elapsedMs: number;
    embedMode: FontEmbedMode;
  }

  let inputMode = $state<InputMode>('upload');
  let theme = $state<'light' | 'dark'>('light');
  let embedMode = $state<FontEmbedMode>('subset');
  let showAdvanced = $state(false);
  let dragDepth = $state(0);

  // Workspace state
  let files = $state<VirtualFile[]>([]);
  let mainPath = $state<string>('');
  let pasteFilename = $state<string>('document.typ');
  let pasteSource = $state<string>('');
  let sysInputs = $state<Record<string, string>>({});
  let newInputKey = $state('');
  let newInputVal = $state('');

  // Engine state
  let client: TypstWordWorkerClient | null = null;
  let enginePhase = $state<EnginePhase>('booting');
  let engineMessage = $state('正在加载引擎…');
  let bootProgress = $state<number | null>(null);
  let fonts = $state<FontFamilyDto[]>([]);
  let packages = $state<string[]>([]);
  let converting = $state(false);
  let errorMessage = $state<string | null>(null);
  let result = $state<ConversionResult | null>(null);

  // Local Font Access API state (Chromium)
  const localFontsSupported = isLocalFontApiSupported();
  let localFontCatalog = $state<LocalFontData[]>([]);
  let localFontState = $state<'idle' | 'requesting' | 'granted' | 'denied'>('idle');
  const loadedLocalFaceKeys = new Set<string>();

  // OpenXML modal state
  let showXRay = $state(false);
  let openXmlPkg = $state<OpenXmlPackageDto | null>(null);
  let irJson = $state<string | null>(null);
  let xrayLoading = $state(false);
  let xrayError = $state<string | null>(null);

  let fileInputEl = $state<HTMLInputElement | null>(null);
  let fontInputEl = $state<HTMLInputElement | null>(null);

  const typFiles = $derived(files.filter((f) => f.path.toLowerCase().endsWith('.typ')));
  const customFontCount = $derived(fonts.filter((f) => !f.is_system_installed).length);
  const hasCloudCjk = $derived(
    fonts.some((f) => {
      const l = f.family.toLowerCase();
      return l.includes('noto serif sc') || l.includes('noto sans cjk') || l.includes('songti');
    }),
  );
  const advancedBadgeCount = $derived(Object.keys(sysInputs).length + (customFontCount > 4 ? customFontCount - 4 : 0));

  const canConvert = $derived(
    enginePhase !== 'booting' &&
      !converting &&
      (inputMode === 'upload' ? typFiles.length > 0 : pasteSource.trim().length > 0),
  );

  function extOf(name: string): string {
    const clean = name.split('/').pop() ?? name;
    const dot = clean.lastIndexOf('.');
    return dot >= 0 ? clean.slice(dot + 1).toLowerCase() : '';
  }

  function formatBytes(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    return `${(n / (1024 * 1024)).toFixed(2)} MB`;
  }

  function applyTheme(next: 'light' | 'dark') {
    theme = next;
    document.documentElement.setAttribute('data-theme', next);
    try {
      localStorage.setItem('typst2word-theme', next);
    } catch {
      // ignore storage errors
    }
  }

  function toggleTheme() {
    applyTheme(theme === 'dark' ? 'light' : 'dark');
  }

  function buildActiveWorkspace(): {
    mainPath: string;
    files: VirtualFile[];
    inputs: Record<string, string>;
  } {
    if (inputMode === 'paste') {
      const cleanName = pasteFilename.trim().replace(/^\/+/, '') || 'document.typ';
      const path = cleanName.endsWith('.typ') ? cleanName : `${cleanName}.typ`;
      const bytes = encoder.encode(pasteSource);
      return {
        mainPath: path,
        files: [{ path, text: pasteSource, bytes }],
        inputs: { ...sysInputs },
      };
    }

    const entry =
      mainPath && files.some((f) => f.path === mainPath)
        ? mainPath
        : (typFiles[0]?.path ?? 'main.typ');
    return {
      mainPath: entry,
      files,
      inputs: { ...sysInputs },
    };
  }

  function deriveOutputFilename(entryPath: string): string {
    const base = (entryPath.split('/').pop() ?? 'document.typ').replace(/\.typ$/i, '');
    return `${base || 'document'}.docx`;
  }

  async function ingestFileList(rawList: FileList | File[]) {
    errorMessage = null;
    const incoming = Array.from(rawList);
    if (incoming.length === 0) return;

    const fontUploads: Array<{ name: string; buffer: ArrayBuffer; isSystemInstalled: boolean }> = [];
    const nextFiles = [...files];
    const upsertFile = (vf: VirtualFile) => {
      const idx = nextFiles.findIndex((f) => f.path === vf.path);
      if (idx >= 0) nextFiles[idx] = vf;
      else nextFiles.push(vf);
    };

    for (const file of incoming) {
      const ext = extOf(file.name);
      if (ext === 'zip') {
        try {
          const zip = await JSZip.loadAsync(await file.arrayBuffer());
          const entries = Object.values(zip.files).filter(
            (e) => !e.dir && !e.name.startsWith('__MACOSX/') && !e.name.split('/').pop()?.startsWith('.'),
          );
          // Strip common top-level directory wrapper if all files share one.
          const firstSegs = new Set(entries.map((e) => (e.name.includes('/') ? e.name.split('/')[0] : '')));
          const commonPrefix =
            firstSegs.size === 1 && !firstSegs.has('') ? `${Array.from(firstSegs)[0]}/` : '';

          for (const entry of entries) {
            const relPath = (
              commonPrefix && entry.name.startsWith(commonPrefix)
                ? entry.name.slice(commonPrefix.length)
                : entry.name
            ).replace(/^\/+/, '');
            if (!relPath) continue;
            const entryExt = extOf(relPath);
            const buf = await entry.async('arraybuffer');
            if (FONT_EXTS.has(entryExt)) {
              fontUploads.push({ name: relPath, buffer: buf, isSystemInstalled: false });
            } else {
              const bytes = new Uint8Array(buf);
              const text = TEXT_EXTS.has(entryExt) ? decoder.decode(bytes) : null;
              upsertFile({ path: relPath, text, bytes });
            }
          }
        } catch (err) {
          errorMessage = `解压 ${file.name} 失败：${err instanceof Error ? err.message : String(err)}`;
        }
        continue;
      }

      const buf = await file.arrayBuffer();
      if (FONT_EXTS.has(ext)) {
        fontUploads.push({ name: file.name, buffer: buf, isSystemInstalled: false });
        continue;
      }

      const relPath = ((file as File & { webkitRelativePath?: string }).webkitRelativePath || file.name).replace(
        /^\/+/,
        '',
      );
      const bytes = new Uint8Array(buf);
      const text = TEXT_EXTS.has(ext) ? decoder.decode(bytes) : null;
      upsertFile({ path: relPath, text, bytes });
    }

    if (fontUploads.length > 0 && client) {
      try {
        const res = await client.addFonts(fontUploads);
        fonts = res.fonts;
      } catch (err) {
        errorMessage = `字体加载失败：${err instanceof Error ? err.message : String(err)}`;
      }
    }

    files = nextFiles;
    const allTyps = nextFiles.filter((f) => f.path.toLowerCase().endsWith('.typ'));
    if (allTyps.length > 0) {
      inputMode = 'upload';
      if (!mainPath || !nextFiles.some((f) => f.path === mainPath)) {
        const preferred =
          allTyps.find((f) => /(^|\/)(main|index)\.typ$/i.test(f.path)) ?? allTyps[0];
        mainPath = preferred.path;
      }
    }
    result = null;
    openXmlPkg = null;
    irJson = null;
  }

  function removeFile(path: string) {
    files = files.filter((f) => f.path !== path);
    if (mainPath === path) {
      const remainingTyp = files.find((f) => f.path.toLowerCase().endsWith('.typ'));
      mainPath = remainingTyp?.path ?? '';
    }
    result = null;
  }

  function clearFiles() {
    files = [];
    mainPath = '';
    result = null;
    errorMessage = null;
    openXmlPkg = null;
    irJson = null;
  }

  function loadShowcasePreset() {
    const preset = STUDIO_PRESETS[0];
    if (!preset) return;
    errorMessage = null;
    result = null;
    openXmlPkg = null;
    irJson = null;
    if (inputMode === 'paste') {
      pasteFilename = preset.mainPath;
      pasteSource = preset.files[preset.mainPath] ?? '';
    } else {
      files = Object.entries(preset.files).map(([path, text]) => ({
        path,
        text,
        bytes: encoder.encode(text),
      }));
      mainPath = preset.mainPath;
    }
    if (preset.inputs) {
      sysInputs = { ...preset.inputs };
    }
  }

  function addSysInput() {
    const k = newInputKey.trim();
    if (!k) return;
    sysInputs = { ...sysInputs, [k]: newInputVal.trim() };
    newInputKey = '';
    newInputVal = '';
  }

  function removeSysInput(key: string) {
    const next = { ...sysInputs };
    delete next[key];
    sysInputs = next;
  }

  async function authorizeLocalFonts(reconvertAfter = false) {
    if (!localFontsSupported || localFontState === 'requesting') return;
    localFontState = 'requesting';
    errorMessage = null;
    try {
      const catalog = await queryLocalFontCatalog();
      localFontCatalog = catalog;
      localFontState = catalog.length > 0 ? 'granted' : 'denied';
      if (reconvertAfter && localFontState === 'granted' && canConvert) {
        await handleConvert();
      }
    } catch {
      localFontState = 'denied';
    }
  }

  async function handleConvert() {
    if (!client || !canConvert) return;
    converting = true;
    errorMessage = null;
    openXmlPkg = null;
    irJson = null;

    try {
      const ws = buildActiveWorkspace();
      if (localFontCatalog.length > 0) {
        const typTexts = ws.files
          .filter((f) => f.path.toLowerCase().endsWith('.typ') && f.text !== null)
          .map((f) => f.text as string);
        engineMessage = '正在匹配本机字体…';
        const matched = await loadMatchingLocalFonts(
          localFontCatalog,
          typTexts,
          loadedLocalFaceKeys,
        );
        if (matched.entries.length > 0) {
          engineMessage = `正在载入本机字体 (${matched.matchedFamilies.slice(0, 3).join(', ')})…`;
          const fontRes = await client.addFonts(matched.entries);
          fonts = fontRes.fonts;
        }
      }
      const out = await client.exportDocx(ws, embedMode);
      if (out.fonts.length > 0) fonts = out.fonts;
      if (out.packages.length > 0) packages = out.packages;
      result = {
        docxBytes: out.docxBytes,
        filename: deriveOutputFilename(ws.mainPath),
        report: out.report,
        elapsedMs: out.elapsedMs,
        embedMode,
      };
      enginePhase = 'ready';
      engineMessage = '就绪';
    } catch (err) {
      errorMessage = err instanceof Error ? err.message : String(err);
      enginePhase = 'error';
      engineMessage = '转换失败';
    } finally {
      converting = false;
    }
  }

  function downloadResult() {
    if (!result) return;
    const copy = new Uint8Array(result.docxBytes.byteLength);
    copy.set(result.docxBytes);
    const blob = new Blob([copy], {
      type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = result.filename;
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }

  async function openXRayModal() {
    showXRay = true;
    if (openXmlPkg || !client) return;
    await refreshXRay();
  }

  async function refreshXRay() {
    if (!client) return;
    xrayLoading = true;
    xrayError = null;
    try {
      const ws = buildActiveWorkspace();
      openXmlPkg = await client.inspectOpenXml(ws, embedMode);
    } catch (err) {
      xrayError = err instanceof Error ? err.message : String(err);
    } finally {
      xrayLoading = false;
    }
  }

  async function loadIrJson() {
    if (!client) return;
    xrayLoading = true;
    xrayError = null;
    try {
      const ws = buildActiveWorkspace();
      const res = await client.inspectIr(ws);
      irJson = res.irJson;
    } catch (err) {
      xrayError = err instanceof Error ? err.message : String(err);
    } finally {
      xrayLoading = false;
    }
  }

  async function handleLoadCloudCjk() {
    if (!client) return;
    try {
      const res = await client.loadCloudCjk();
      fonts = res.fonts;
    } catch (err) {
      errorMessage = err instanceof Error ? err.message : String(err);
    }
  }

  function onFileInputChange(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    if (input.files && input.files.length > 0) {
      void ingestFileList(input.files);
      input.value = '';
    }
  }

  async function onFontInputChange(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    if (!input.files || input.files.length === 0 || !client) return;
    const entries: Array<{ name: string; buffer: ArrayBuffer; isSystemInstalled: boolean }> = [];
    for (const f of Array.from(input.files)) {
      entries.push({ name: f.name, buffer: await f.arrayBuffer(), isSystemInstalled: false });
    }
    input.value = '';
    try {
      const res = await client.addFonts(entries);
      fonts = res.fonts;
    } catch (err) {
      errorMessage = err instanceof Error ? err.message : String(err);
    }
  }

  function handleWindowDragEnter(e: DragEvent) {
    if (e.dataTransfer?.types.includes('Files')) {
      dragDepth += 1;
    }
  }
  function handleWindowDragLeave() {
    dragDepth = Math.max(0, dragDepth - 1);
  }
  function handleWindowDragOver(e: DragEvent) {
    if (e.dataTransfer?.types.includes('Files')) {
      e.preventDefault();
    }
  }
  function handleWindowDrop(e: DragEvent) {
    e.preventDefault();
    dragDepth = 0;
    if (e.dataTransfer?.files && e.dataTransfer.files.length > 0) {
      void ingestFileList(e.dataTransfer.files);
    }
  }

  function buildMetricChips(report: ConversionReport): Array<{ label: string; value: number }> {
    const items: Array<{ label: string; value: number }> = [
      { label: '页', value: report.pages_count },
      { label: '标题', value: report.headings_count },
      { label: 'OMML 公式', value: report.equations_count },
      { label: '表格', value: report.tables_count },
      { label: '矢量图', value: report.shapes_count + report.cetz_canvases_count },
      { label: '代码块', value: report.code_blocks_count },
      { label: '图片', value: report.images_count },
      { label: '脚注', value: report.footnotes_count },
    ];
    return items.filter((item) => item.value > 0);
  }

  onMount(() => {
    try {
      const saved = localStorage.getItem('typst2word-theme');
      if (saved === 'dark' || saved === 'light') {
        applyTheme(saved);
      } else if (window.matchMedia?.('(prefers-color-scheme: dark)').matches) {
        applyTheme('dark');
      }
    } catch {
      // ignore
    }

    client = new TypstWordWorkerClient({
      onBooted: ({ fonts: f, packages: p }) => {
        fonts = f;
        packages = p;
        enginePhase = 'ready';
        engineMessage = '就绪';
        bootProgress = 100;
      },
      onStatus: (phase, message, progress) => {
        enginePhase = phase;
        engineMessage = message;
        if (phase === 'booting' && typeof progress === 'number') {
          bootProgress = progress;
        }
      },
      onFontsUpdated: (f) => {
        fonts = f;
      },
      onPackagesUpdated: (p) => {
        packages = p;
      },
    });

    void client.boot().catch((err) => {
      enginePhase = 'error';
      engineMessage = '引擎启动失败';
      errorMessage = err instanceof Error ? err.message : String(err);
    });
  });

  onDestroy(() => {
    client?.destroy();
    client = null;
  });
</script>

<svelte:window
  ondragenter={handleWindowDragEnter}
  ondragleave={handleWindowDragLeave}
  ondragover={handleWindowDragOver}
  ondrop={handleWindowDrop}
/>

<input
  bind:this={fileInputEl}
  type="file"
  multiple
  accept=".typ,.zip,.png,.jpg,.jpeg,.gif,.svg,.bib,.csv,.json,.toml,.yaml,.yml,.txt,.ttf,.otf,.ttc,.otc"
  onchange={onFileInputChange}
  hidden
/>
<input
  bind:this={fontInputEl}
  type="file"
  multiple
  accept=".ttf,.otf,.ttc,.otc"
  onchange={onFontInputChange}
  hidden
/>

{#if dragDepth > 0}
  <div class="drop-overlay" aria-hidden="true">
    <div class="drop-overlay-inner">
      <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
        <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
        <polyline points="17 8 12 3 7 8" />
        <line x1="12" y1="3" x2="12" y2="15" />
      </svg>
      <span>松开以导入 .typ、.zip 工程包、图片或字体</span>
    </div>
  </div>
{/if}

<div class="page">
  <header class="topbar">
    <div class="brand">
      <span class="brand-mark">T→W</span>
      <span class="brand-name">typst2word</span>
      <span
        class="status-pill"
        class:ready={enginePhase === 'ready'}
        class:busy={enginePhase === 'booting' || converting}
        class:err={enginePhase === 'error'}
      >
        <span class="status-dot"></span>
        {#if enginePhase === 'booting'}
          {#if bootProgress !== null}
            加载引擎 {bootProgress}%
          {:else}
            正在加载引擎…
          {/if}
        {:else if enginePhase === 'error'}
          引擎异常
        {:else}
          本地 WASM
        {/if}
      </span>
    </div>

    <div class="topbar-actions">
      <button
        type="button"
        class="icon-btn"
        onclick={toggleTheme}
        title={theme === 'dark' ? '切换浅色模式' : '切换深色模式'}
        aria-label="切换主题"
      >
        {#if theme === 'dark'}
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="4" />
            <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41" />
          </svg>
        {:else}
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
          </svg>
        {/if}
      </button>
      <a
        class="icon-btn"
        href="https://github.com/WangHaoZhengMing/typst2word"
        target="_blank"
        rel="noreferrer"
        title="GitHub 仓库"
        aria-label="GitHub 仓库"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
          <path
            d="M12 .5C5.65.5.5 5.65.5 12c0 5.08 3.29 9.39 7.86 10.91.58.11.79-.25.79-.56 0-.28-.01-1.02-.02-2-3.2.7-3.88-1.54-3.88-1.54-.52-1.33-1.28-1.68-1.28-1.68-1.05-.72.08-.7.08-.7 1.16.08 1.77 1.19 1.77 1.19 1.03 1.77 2.7 1.26 3.36.96.1-.75.4-1.26.73-1.55-2.55-.29-5.24-1.28-5.24-5.68 0-1.25.45-2.28 1.18-3.08-.12-.29-.51-1.46.11-3.04 0 0 .97-.31 3.16 1.18a10.95 10.95 0 0 1 5.76 0c2.2-1.49 3.16-1.18 3.16-1.18.63 1.58.23 2.75.11 3.04.74.8 1.18 1.83 1.18 3.08 0 4.41-2.69 5.39-5.25 5.67.41.36.78 1.06.78 2.14 0 1.54-.01 2.79-.01 3.17 0 .31.21.67.8.56A11.51 11.51 0 0 0 23.5 12C23.5 5.65 18.35.5 12 .5Z"
          />
        </svg>
      </a>
    </div>
  </header>

  <main class="container">
    <section class="hero">
      <h1>Typst 转 Word</h1>
      <p>高保真输出原生 OMML 公式、矢量图形与嵌入字体。纯浏览器本地运行，文件绝不上传。</p>
    </section>

    <section class="card converter-card">
      <div class="card-head">
        <div class="segmented" role="tablist" aria-label="输入方式">
          <button
            type="button"
            role="tab"
            aria-selected={inputMode === 'upload'}
            class:active={inputMode === 'upload'}
            onclick={() => {
              inputMode = 'upload';
              errorMessage = null;
            }}
          >
            上传文件
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={inputMode === 'paste'}
            class:active={inputMode === 'paste'}
            onclick={() => {
              inputMode = 'paste';
              errorMessage = null;
            }}
          >
            粘贴代码
          </button>
        </div>

        <button type="button" class="ghost-link" onclick={loadShowcasePreset}>
          载入示例
        </button>
      </div>

      <div class="card-body">
        {#if inputMode === 'upload'}
          {#if files.length === 0}
            <button
              type="button"
              class="dropzone"
              onclick={() => fileInputEl?.click()}
            >
              <div class="dropzone-icon">
                <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75">
                  <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                  <polyline points="17 8 12 3 7 8" />
                  <line x1="12" y1="3" x2="12" y2="15" />
                </svg>
              </div>
              <div class="dropzone-title">拖放 .typ 文件或 .zip 工程包到此处，或点击选择</div>
              <div class="dropzone-sub">支持同时附加图片（PNG / JPG / SVG）与自定义字体（TTF / OTF / TTC）</div>
            </button>
          {:else}
            <div class="file-list-box">
              <div class="file-list-header">
                <span>已载入 {files.length} 个文件</span>
                <div class="file-list-actions">
                  <button type="button" class="ghost-link" onclick={() => fileInputEl?.click()}>
                    + 添加文件 / 字体
                  </button>
                  <button type="button" class="ghost-link muted" onclick={clearFiles}>
                    清空
                  </button>
                </div>
              </div>
              <ul class="file-list">
                {#each files as file (file.path)}
                  {@const isTyp = file.path.toLowerCase().endsWith('.typ')}
                  {@const isEntry = file.path === mainPath}
                  <li class="file-row">
                    <div class="file-meta">
                      <span class="file-badge" class:typ={isTyp}>
                        {isTyp ? 'TYP' : extOf(file.path).toUpperCase() || 'FILE'}
                      </span>
                      <span class="file-path" title={file.path}>{file.path}</span>
                      <span class="file-size">{formatBytes(file.bytes.byteLength)}</span>
                    </div>
                    <div class="file-row-right">
                      {#if isTyp && isEntry}
                        <span class="entry-tag">入口</span>
                      {:else if isTyp}
                        <button
                          type="button"
                          class="entry-switch"
                          onclick={() => {
                            mainPath = file.path;
                            result = null;
                          }}
                        >
                          设为入口
                        </button>
                      {/if}
                      <button
                        type="button"
                        class="remove-btn"
                        onclick={() => removeFile(file.path)}
                        aria-label={`移除 ${file.path}`}
                      >
                        ×
                      </button>
                    </div>
                  </li>
                {/each}
              </ul>
              {#if typFiles.length === 0}
                <div class="inline-hint warn">
                  尚未包含 `.typ` 入口文件，请继续添加 `.typ` 文件。
                </div>
              {/if}
            </div>
          {/if}
        {:else}
          <div class="paste-box">
            <div class="paste-meta">
              <label for="paste-filename">文件名</label>
              <input
                id="paste-filename"
                type="text"
                bind:value={pasteFilename}
                placeholder="document.typ"
              />
            </div>
            <textarea
              class="paste-textarea"
              bind:value={pasteSource}
              spellcheck="false"
              placeholder={'= 示例标题\n\n在此粘贴 Typst 代码，支持数学公式 $E = m c^2$、表格与 CeTZ 图形…'}
            ></textarea>
          </div>
        {/if}
      </div>

      <div class="options-bar">
        <div class="option-group">
          <span class="option-label">字体嵌入</span>
          <div class="segmented small" role="radiogroup" aria-label="字体嵌入模式">
            <button
              type="button"
              role="radio"
              aria-checked={embedMode === 'subset'}
              class:active={embedMode === 'subset'}
              onclick={() => (embedMode = 'subset')}
              title="仅嵌入文档实际使用的字符，体积小（推荐）"
            >
              子集
            </button>
            <button
              type="button"
              role="radio"
              aria-checked={embedMode === 'full'}
              class:active={embedMode === 'full'}
              onclick={() => (embedMode = 'full')}
              title="嵌入完整字体文件，Word 中任意编辑不回退"
            >
              完整
            </button>
            <button
              type="button"
              role="radio"
              aria-checked={embedMode === 'none'}
              class:active={embedMode === 'none'}
              onclick={() => (embedMode = 'none')}
              title="不嵌入字体，生成最小体积 .docx"
            >
              不嵌入
            </button>
          </div>
        </div>

        <div class="options-right">
          {#if localFontsSupported}
            <button
              type="button"
              class="local-font-btn"
              class:granted={localFontState === 'granted'}
              onclick={() => authorizeLocalFonts(false)}
              title={localFontState === 'granted'
                ? `已授权 ${localFontCatalog.length} 个本机字体，转换时自动按需加载`
                : '授权读取本机已安装字体（转换时仅按需加载文档用到的字体）'}
            >
              {#if localFontState === 'granted'}
                <span class="lf-dot"></span>
                <span>本机字体 ({localFontCatalog.length})</span>
              {:else if localFontState === 'requesting'}
                <span>授权中…</span>
              {:else if localFontState === 'denied'}
                <span>本机字体未授权</span>
              {:else}
                <span>使用本机字体</span>
              {/if}
            </button>
          {/if}

          <button
            type="button"
            class="advanced-toggle"
            class:open={showAdvanced}
            onclick={() => (showAdvanced = !showAdvanced)}
          >
            <span>高级选项</span>
            {#if advancedBadgeCount > 0}
              <span class="mini-badge">{advancedBadgeCount}</span>
            {/if}
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              class="chevron"
            >
              <polyline points="6 9 12 15 18 9" />
            </svg>
          </button>
        </div>
      </div>

      {#if showAdvanced}
        <div class="advanced-panel">
          <div class="adv-row">
            <div class="adv-title">
              <span>自定义字体</span>
              <span class="adv-meta">当前已载入 {fonts.length} 个字体族</span>
            </div>
            <div class="adv-controls">
              {#if localFontsSupported}
                <button
                  type="button"
                  class="secondary-btn"
                  onclick={() => authorizeLocalFonts(false)}
                >
                  {localFontState === 'granted'
                    ? `已授权本机字体 (${localFontCatalog.length})`
                    : '授权读取本机字体'}
                </button>
              {/if}
              <button type="button" class="secondary-btn" onclick={() => fontInputEl?.click()}>
                上传 .ttf / .otf / .ttc
              </button>
              {#if !hasCloudCjk}
                <button type="button" class="secondary-btn" onclick={handleLoadCloudCjk}>
                  加载思源宋体
                </button>
              {/if}
            </div>
            {#if !localFontsSupported}
              <div class="adv-meta">
                当前浏览器不支持自动读取本机字体（Chrome / Edge 支持），可手动上传字体文件。
              </div>
            {/if}
          </div>

          <div class="adv-row">
            <div class="adv-title">
              <span>模板变量 (sys.inputs)</span>
              <span class="adv-meta">供 Typst 通过 sys.inputs 读取</span>
            </div>
            <div class="kv-form">
              <input type="text" bind:value={newInputKey} placeholder="key" />
              <input
                type="text"
                bind:value={newInputVal}
                placeholder="value"
                onkeydown={(e) => e.key === 'Enter' && addSysInput()}
              />
              <button type="button" class="secondary-btn" onclick={addSysInput}>添加</button>
            </div>
            {#if Object.keys(sysInputs).length > 0}
              <div class="kv-tags">
                {#each Object.entries(sysInputs) as [k, v]}
                  <span class="kv-tag">
                    <code>{k}={v}</code>
                    <button type="button" onclick={() => removeSysInput(k)} aria-label={`移除 ${k}`}>
                      ×
                    </button>
                  </span>
                {/each}
              </div>
            {/if}
          </div>

          {#if packages.length > 0}
            <div class="adv-row">
              <div class="adv-title">
                <span>已缓存 @preview 包</span>
              </div>
              <div class="kv-tags">
                {#each packages as pkg}
                  <span class="kv-tag"><code>{pkg}</code></span>
                {/each}
              </div>
            </div>
          {/if}
        </div>
      {/if}

      <div class="cta-row">
        <button
          type="button"
          class="primary-btn"
          class:booting={enginePhase === 'booting'}
          disabled={!canConvert}
          onclick={handleConvert}
        >
          {#if converting}
            <span class="spinner"></span>
            <span>{engineMessage || '正在转换…'}</span>
          {:else if enginePhase === 'booting'}
            <span class="spinner"></span>
            <span>{engineMessage || '正在初始化 WASM 引擎…'}</span>
            {#if bootProgress !== null}
              <span class="btn-progress" style={`width: ${bootProgress}%`}></span>
            {/if}
          {:else}
            <span>转换为 .docx</span>
          {/if}
        </button>
      </div>
    </section>

    {#if errorMessage}
      <section class="alert alert-danger" role="alert">
        <div class="alert-title">转换失败</div>
        <pre class="alert-pre">{errorMessage}</pre>
      </section>
    {/if}

    {#if result}
      {@const chips = buildMetricChips(result.report)}
      {@const warnings = [
        ...(result.report.compiler_warnings ?? []),
        ...(result.report.font_fallback_warnings ?? []),
      ]}
      {@const embedNotes = result.report.font_embed_notes ?? []}

      <section class="card result-card">
        <div class="result-head">
          <div class="result-file">
            <span class="check-circle" aria-hidden="true">
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6">
                <polyline points="20 6 9 17 4 12" />
              </svg>
            </span>
            <div>
              <div class="result-name">{result.filename}</div>
              <div class="result-sub">
                {formatBytes(result.docxBytes.byteLength)} · {result.elapsedMs} ms
              </div>
            </div>
          </div>

          <button type="button" class="download-btn" onclick={downloadResult}>
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
              <polyline points="7 10 12 15 17 10" />
              <line x1="12" y1="15" x2="12" y2="3" />
            </svg>
            <span>下载 .docx</span>
          </button>
        </div>

        {#if chips.length > 0}
          <div class="metrics-strip">
            {#each chips as chip}
              <span class="metric-chip">
                <strong>{chip.value}</strong>
                <span>{chip.label}</span>
              </span>
            {/each}
          </div>
        {/if}

        {#if embedNotes.length > 0}
          <div class="note-list info">
            {#each embedNotes as note}
              <div class="note-item">{note}</div>
            {/each}
          </div>
        {/if}

        {#if warnings.length > 0}
          <div class="note-list warn">
            {#each warnings as w}
              <div class="note-item">{w}</div>
            {/each}
            {#if (result.report.font_fallback_warnings?.length ?? 0) > 0}
              <div class="inline-actions-row">
                {#if localFontsSupported && localFontState !== 'granted'}
                  <button
                    type="button"
                    class="inline-action"
                    onclick={() => authorizeLocalFonts(true)}
                  >
                    授权读取本机字体并重新转换 →
                  </button>
                {/if}
                <button
                  type="button"
                  class="inline-action"
                  onclick={() => fontInputEl?.click()}
                >
                  手动上传字体 (.ttf / .otf) →
                </button>
              </div>
            {/if}
          </div>
        {/if}

        {#if result.report.unsupported_elements.length > 0}
          <div class="note-list warn">
            <div class="note-item">
              包含 {result.report.unsupported_elements.length} 处不支持的元素（已在文档中保留占位标记）
            </div>
          </div>
        {/if}

        <div class="result-foot">
          <button type="button" class="ghost-link" onclick={openXRayModal}>
            检视 OpenXML 包结构 →
          </button>
        </div>
      </section>
    {/if}
  </main>

  <footer class="footer">
    <span>Typst 0.15 · 纯本地 WebAssembly</span>
    <span class="dot">·</span>
    <a
      href="https://github.com/WangHaoZhengMing/typst2word/releases"
      target="_blank"
      rel="noreferrer"
    >
      下载桌面 CLI 版本
    </a>
  </footer>
</div>

{#if showXRay}
  <div
    class="modal-backdrop"
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) showXRay = false;
    }}
  >
    <div class="modal" role="dialog" aria-modal="true" aria-label="OpenXML 包结构">
      <div class="modal-head">
        <span>OpenXML 包结构</span>
        <button
          type="button"
          class="icon-btn"
          onclick={() => (showXRay = false)}
          aria-label="关闭"
        >
          ×
        </button>
      </div>
      <div class="modal-body">
        <XRayInspector
          {openXmlPkg}
          {irJson}
          loading={xrayLoading}
          error={xrayError}
          initialFilterTag=""
          onRefresh={refreshXRay}
          onLoadIr={loadIrJson}
        />
      </div>
    </div>
  </div>
{/if}

<style>
  .page {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    padding: 0 20px;
  }

  .topbar {
    width: 100%;
    max-width: 680px;
    margin: 0 auto;
    height: 60px;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .brand-mark {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: 24px;
    padding: 0 7px;
    border-radius: 6px;
    background: var(--primary-bg);
    color: var(--primary-text);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.02em;
  }

  .brand-name {
    font-weight: 600;
    font-size: 15px;
    letter-spacing: -0.02em;
  }

  .status-pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 2px 8px;
    border-radius: 999px;
    font-size: 11.5px;
    color: var(--text-muted);
    background: var(--surface-subtle);
  }

  .status-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--text-muted);
  }

  .status-pill.ready .status-dot {
    background: var(--success);
  }

  .status-pill.busy .status-dot {
    background: var(--warn);
    animation: pulse 1.2s ease-in-out infinite;
  }

  .status-pill.err .status-dot {
    background: var(--danger);
  }

  .topbar-actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .icon-btn {
    width: 32px;
    height: 32px;
    border-radius: 8px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-secondary);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    text-decoration: none;
    font-size: 18px;
    transition: background 0.15s ease, color 0.15s ease;
  }

  .icon-btn:hover {
    background: var(--surface-subtle);
    color: var(--text);
  }

  .container {
    width: 100%;
    max-width: 680px;
    margin: 28px auto 48px;
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .hero {
    text-align: center;
    margin-bottom: 10px;
  }

  .hero h1 {
    margin: 0;
    font-size: 28px;
    font-weight: 650;
    letter-spacing: -0.03em;
    line-height: 1.2;
  }

  .hero p {
    margin: 8px 0 0;
    font-size: 14px;
    color: var(--text-secondary);
  }

  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 14px;
    box-shadow: var(--shadow-card);
    overflow: hidden;
  }

  .card-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
  }

  .segmented {
    display: inline-flex;
    padding: 3px;
    border-radius: 9px;
    background: var(--surface-subtle);
    gap: 2px;
  }

  .segmented button {
    border: none;
    background: transparent;
    padding: 5px 12px;
    border-radius: 6px;
    font-size: 13px;
    font-weight: 500;
    color: var(--text-secondary);
    transition: background 0.15s ease, color 0.15s ease, box-shadow 0.15s ease;
  }

  .segmented.small button {
    padding: 3px 10px;
    font-size: 12px;
  }

  .segmented button.active {
    background: var(--surface);
    color: var(--text);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.08);
  }

  .ghost-link {
    border: none;
    background: transparent;
    padding: 4px 6px;
    border-radius: 6px;
    font-size: 12.5px;
    font-weight: 500;
    color: var(--text-secondary);
    transition: color 0.15s ease, background 0.15s ease;
  }

  .ghost-link:hover {
    color: var(--text);
    background: var(--surface-subtle);
  }

  .ghost-link.muted {
    color: var(--text-muted);
  }

  .card-body {
    padding: 16px;
  }

  .dropzone {
    width: 100%;
    min-height: 196px;
    border: 1.5px dashed var(--border-strong);
    border-radius: 11px;
    background: var(--bg);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 24px;
    text-align: center;
    transition: border-color 0.15s ease, background 0.15s ease;
  }

  .dropzone:hover {
    border-color: var(--text-muted);
    background: var(--surface-subtle);
  }

  .dropzone-icon {
    width: 44px;
    height: 44px;
    border-radius: 12px;
    background: var(--surface);
    border: 1px solid var(--border);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
    margin-bottom: 4px;
  }

  .dropzone-title {
    font-size: 14px;
    font-weight: 500;
    color: var(--text);
  }

  .dropzone-sub {
    font-size: 12.5px;
    color: var(--text-muted);
  }

  .file-list-box {
    border: 1px solid var(--border);
    border-radius: 10px;
    overflow: hidden;
  }

  .file-list-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px;
    background: var(--surface-subtle);
    font-size: 12px;
    color: var(--text-secondary);
    border-bottom: 1px solid var(--border);
  }

  .file-list-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .file-list {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 220px;
    overflow-y: auto;
  }

  .file-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 9px 12px;
    border-bottom: 1px solid var(--border);
  }

  .file-row:last-child {
    border-bottom: none;
  }

  .file-meta {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .file-badge {
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 600;
    padding: 2px 5px;
    border-radius: 4px;
    background: var(--surface-subtle);
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .file-badge.typ {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .file-path {
    font-size: 13px;
    font-family: var(--font-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .file-size {
    font-size: 11.5px;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .file-row-right {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .entry-tag {
    font-size: 11px;
    font-weight: 600;
    padding: 2px 7px;
    border-radius: 999px;
    background: var(--primary-bg);
    color: var(--primary-text);
  }

  .entry-switch {
    border: 1px solid var(--border);
    background: transparent;
    padding: 2px 7px;
    border-radius: 999px;
    font-size: 11px;
    color: var(--text-secondary);
  }

  .entry-switch:hover {
    border-color: var(--border-strong);
    color: var(--text);
  }

  .remove-btn {
    border: none;
    background: transparent;
    width: 22px;
    height: 22px;
    border-radius: 6px;
    font-size: 15px;
    color: var(--text-muted);
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .remove-btn:hover {
    background: var(--surface-subtle);
    color: var(--text);
  }

  .inline-hint.warn {
    padding: 8px 12px;
    font-size: 12px;
    color: var(--warn);
    background: var(--warn-bg);
    border-top: 1px solid var(--warn-border);
  }

  .paste-box {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .paste-meta {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .paste-meta input {
    width: 180px;
    padding: 4px 8px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .paste-textarea {
    width: 100%;
    height: 260px;
    resize: vertical;
    padding: 12px 14px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--bg);
    font-family: var(--font-mono);
    font-size: 12.5px;
    line-height: 1.6;
    outline: none;
    transition: border-color 0.15s ease;
  }

  .paste-textarea:focus {
    border-color: var(--text-muted);
  }

  .options-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 12px;
    padding: 12px 16px;
    border-top: 1px solid var(--border);
    background: var(--bg);
  }

  .option-group {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .option-label {
    font-size: 12.5px;
    color: var(--text-secondary);
    font-weight: 500;
  }

  .options-right {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .local-font-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text-secondary);
    padding: 4px 9px;
    border-radius: 7px;
    font-size: 12px;
    font-weight: 500;
    transition: border-color 0.15s ease, color 0.15s ease, background 0.15s ease;
  }

  .local-font-btn:hover {
    border-color: var(--border-strong);
    color: var(--text);
  }

  .local-font-btn.granted {
    border-color: var(--success-border);
    background: var(--success-bg);
    color: var(--success);
  }

  .lf-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--success);
  }

  .inline-actions-row {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    margin-top: 4px;
  }

  .advanced-toggle {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    border: none;
    background: transparent;
    font-size: 12.5px;
    color: var(--text-secondary);
    padding: 4px 6px;
    border-radius: 6px;
  }

  .advanced-toggle:hover {
    color: var(--text);
    background: var(--surface-subtle);
  }

  .advanced-toggle .chevron {
    transition: transform 0.15s ease;
  }

  .advanced-toggle.open .chevron {
    transform: rotate(180deg);
  }

  .mini-badge {
    min-width: 16px;
    height: 16px;
    padding: 0 5px;
    border-radius: 999px;
    background: var(--primary-bg);
    color: var(--primary-text);
    font-size: 10.5px;
    font-weight: 600;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .advanced-panel {
    padding: 14px 16px;
    border-top: 1px solid var(--border);
    background: var(--bg);
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .adv-row {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .adv-title {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    font-size: 12.5px;
    font-weight: 500;
  }

  .adv-meta {
    font-size: 11.5px;
    font-weight: 400;
    color: var(--text-muted);
  }

  .adv-controls {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .secondary-btn {
    border: 1px solid var(--border);
    background: var(--surface);
    padding: 5px 10px;
    border-radius: 7px;
    font-size: 12px;
    font-weight: 500;
    color: var(--text);
    transition: border-color 0.15s ease, background 0.15s ease;
  }

  .secondary-btn:hover {
    border-color: var(--border-strong);
    background: var(--surface-subtle);
  }

  .kv-form {
    display: flex;
    gap: 8px;
  }

  .kv-form input {
    flex: 1;
    min-width: 0;
    padding: 5px 9px;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--surface);
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .kv-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .kv-tag {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 2px 8px;
    border-radius: 6px;
    background: var(--surface);
    border: 1px solid var(--border);
    font-size: 11.5px;
    font-family: var(--font-mono);
  }

  .kv-tag button {
    border: none;
    background: transparent;
    padding: 0;
    color: var(--text-muted);
    font-size: 13px;
  }

  .cta-row {
    padding: 14px 16px 16px;
    border-top: 1px solid var(--border);
  }

  .primary-btn {
    position: relative;
    overflow: hidden;
    width: 100%;
    height: 42px;
    border: none;
    border-radius: 10px;
    background: var(--primary-bg);
    color: var(--primary-text);
    font-size: 14px;
    font-weight: 600;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    transition: background 0.15s ease, opacity 0.15s ease;
  }

  .primary-btn.booting {
    opacity: 0.78;
  }

  .btn-progress {
    position: absolute;
    left: 0;
    bottom: 0;
    height: 3px;
    background: var(--accent);
    transition: width 0.16s ease-out;
  }

  .primary-btn:hover:not(:disabled) {
    background: var(--primary-bg-hover);
  }

  .primary-btn:disabled {
    opacity: 0.42;
    cursor: not-allowed;
  }

  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid currentColor;
    border-right-color: transparent;
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }

  .alert {
    padding: 12px 14px;
    border-radius: 12px;
    border: 1px solid var(--danger-border);
    background: var(--danger-bg);
    color: var(--danger);
  }

  .alert-title {
    font-size: 13px;
    font-weight: 600;
    margin-bottom: 4px;
  }

  .alert-pre {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 12px;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .result-card {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .result-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }

  .result-file {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }

  .check-circle {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    background: var(--success-bg);
    color: var(--success);
    border: 1px solid var(--success-border);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .result-name {
    font-size: 14.5px;
    font-weight: 600;
    font-family: var(--font-mono);
  }

  .result-sub {
    font-size: 12px;
    color: var(--text-muted);
  }

  .download-btn {
    height: 36px;
    padding: 0 14px;
    border: none;
    border-radius: 9px;
    background: var(--success);
    color: #ffffff;
    font-size: 13px;
    font-weight: 600;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    transition: filter 0.15s ease;
  }

  .download-btn:hover {
    filter: brightness(0.93);
  }

  .metrics-strip {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .metric-chip {
    display: inline-flex;
    align-items: baseline;
    gap: 4px;
    padding: 3px 9px;
    border-radius: 6px;
    background: var(--surface-subtle);
    font-size: 12px;
    color: var(--text-secondary);
  }

  .metric-chip strong {
    color: var(--text);
    font-weight: 600;
  }

  .note-list {
    padding: 10px 12px;
    border-radius: 9px;
    font-size: 12.5px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .note-list.info {
    background: var(--surface-subtle);
    color: var(--text-secondary);
  }

  .note-list.warn {
    background: var(--warn-bg);
    border: 1px solid var(--warn-border);
    color: var(--warn);
  }

  .inline-action {
    align-self: flex-start;
    margin-top: 4px;
    border: none;
    background: transparent;
    padding: 0;
    font-size: 12px;
    font-weight: 600;
    color: inherit;
    text-decoration: underline;
  }

  .result-foot {
    display: flex;
    justify-content: flex-end;
    padding-top: 4px;
    border-top: 1px solid var(--border);
  }

  .footer {
    width: 100%;
    max-width: 680px;
    margin: 0 auto 24px;
    text-align: center;
    font-size: 12px;
    color: var(--text-muted);
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
  }

  .footer a {
    color: var(--text-secondary);
    text-decoration: none;
  }

  .footer a:hover {
    text-decoration: underline;
  }

  .drop-overlay {
    position: fixed;
    inset: 0;
    z-index: 60;
    background: rgba(9, 9, 11, 0.45);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    pointer-events: none;
  }

  .drop-overlay-inner {
    padding: 28px 36px;
    border-radius: 16px;
    background: var(--surface);
    border: 1.5px dashed var(--text);
    box-shadow: var(--shadow-modal);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    font-size: 15px;
    font-weight: 600;
  }

  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    background: rgba(9, 9, 11, 0.5);
    backdrop-filter: blur(3px);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 20px;
  }

  .modal {
    width: 100%;
    max-width: 920px;
    height: min(82vh, 720px);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 14px;
    box-shadow: var(--shadow-modal);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .modal-head {
    height: 46px;
    padding: 0 14px;
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 13.5px;
    font-weight: 600;
    flex-shrink: 0;
  }

  .modal-body {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  .modal-body > :global(*) {
    width: 100%;
    height: 100%;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.35;
    }
  }
</style>
