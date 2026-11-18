<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import JSZip from 'jszip';
  import Toolbar from './lib/components/Toolbar.svelte';
  import WorkspaceSidebar from './lib/components/WorkspaceSidebar.svelte';
  import TypstEditor from './lib/components/TypstEditor.svelte';
  import PageCanvas from './lib/components/PageCanvas.svelte';
  import TelemetryView from './lib/components/TelemetryView.svelte';
  import XRayInspector from './lib/components/XRayInspector.svelte';
  import { STUDIO_PRESETS } from './lib/presets';
  import { TypstWordWorkerClient } from './lib/worker-client';
  import type {
    ConversionReport,
    EngineStatus,
    FontFamilyDto,
    OpenXmlPackageDto,
    StageMode,
    StudioPreset,
    VirtualFile,
  } from './lib/types';

  const TEXT_EXTENSIONS = new Set([
    'typ',
    'bib',
    'csv',
    'txt',
    'md',
    'json',
    'toml',
    'yaml',
    'yml',
    'xml',
    'svg',
  ]);

  const PREFERRED_LOCAL_FAMILIES = new Set([
    'songti sc',
    'stsong',
    'pingfang sc',
    'heiti sc',
    'stheiti',
    'kaiti sc',
    'stkaiti',
    'fangsong',
    'stfangsong',
    'simsun',
    'nsimsun',
    'simhei',
    'microsoft yahei',
    'dengxian',
    'kaiti',
    'times new roman',
    'arial',
    'calibri',
    'cambria',
    'cambria math',
    'georgia',
    'helvetica',
    'consolas',
    'courier new',
    'menlo',
    'sf pro text',
    'noto serif sc',
    'noto sans sc',
  ]);

  const encoder = new TextEncoder();
  const decoder = new TextDecoder('utf-8');

  function isTextFilePath(path: string): boolean {
    const ext = path.split('.').pop()?.toLowerCase() ?? '';
    return TEXT_EXTENSIONS.has(ext);
  }

  function makeTextVirtualFile(path: string, content: string): VirtualFile {
    return {
      path,
      text: content,
      bytes: encoder.encode(content),
    };
  }

  function presetToVirtualFiles(preset: StudioPreset): VirtualFile[] {
    return Object.entries(preset.files).map(([path, text]) => makeTextVirtualFile(path, text));
  }

  const initialPreset = STUDIO_PRESETS[0];

  let theme = $state<'light' | 'dark'>('light');
  let sidebarOpen = $state(true);
  let activePresetId = $state(initialPreset.id);
  let files = $state<VirtualFile[]>(presetToVirtualFiles(initialPreset));
  let mainFilePath = $state(initialPreset.mainPath);
  let activeFilePath = $state(initialPreset.mainPath);
  let sysInputs = $state<Record<string, string>>({ ...(initialPreset.inputs ?? {}) });

  let stageMode = $state<StageMode>('preview');
  let mobilePane = $state<'editor' | 'stage'>('stage');
  let splitRatio = $state(0.47);
  let isDraggingSplit = $state(false);
  let isDragOverWindow = $state(false);

  let engineStatus = $state<EngineStatus>({
    phase: 'booting',
    message: '正在启动 WASM 编译器…',
  });
  let pagesSvg = $state<string[]>([]);
  let report = $state<ConversionReport | null>(null);
  let documentTitle = $state<string | null>(null);
  let compilerWarnings = $state<string[]>([]);
  let compileError = $state<string | null>(null);
  let fonts = $state<FontFamilyDto[]>([]);
  let loadedPackages = $state<string[]>([]);
  let isExporting = $state(false);

  // OpenXML / IR X-Ray state
  let openXmlPkg = $state<OpenXmlPackageDto | null>(null);
  let irJson = $state<string | null>(null);
  let xrayLoading = $state(false);
  let xrayError = $state<string | null>(null);
  let xrayFilterTag = $state('');

  // Toast notification
  let toastMessage = $state<string | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | null = null;

  let client: TypstWordWorkerClient | null = null;
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;
  let compileSeq = 0;

  const supportsLocalFontAccess =
    typeof window !== 'undefined' && 'queryLocalFonts' in window;

  const activeFile = $derived(
    files.find((f) => f.path === activeFilePath) ?? files[0] ?? null,
  );

  function showToast(msg: string) {
    toastMessage = msg;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      toastMessage = null;
    }, 3200);
  }

  function applyTheme(next: 'light' | 'dark') {
    theme = next;
    document.documentElement.setAttribute('data-theme', next);
    try {
      localStorage.setItem('typst2word-theme', next);
    } catch {
      // Ignore storage restrictions
    }
  }

  function scheduleCompile(delayMs = 160) {
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      void runPreviewCompile();
    }, delayMs);
  }

  async function runPreviewCompile() {
    if (!client) return;
    const seq = ++compileSeq;

    try {
      const res = await client.syncAndPreview({
        mainPath: mainFilePath,
        files,
        inputs: sysInputs,
      });
      if (seq !== compileSeq) return;

      fonts = res.fonts;
      loadedPackages = res.packages;

      if (res.preview.status === 'ok') {
        pagesSvg = res.preview.pages_svg;
        report = res.preview.report;
        documentTitle = res.preview.document_title;
        compilerWarnings = res.preview.compiler_warnings;
        compileError = null;
        engineStatus = {
          phase: 'ready',
          message: '就绪',
          elapsedMs: res.elapsedMs,
        };
        if (stageMode === 'xray') {
          void refreshXRay();
        }
      } else {
        compileError = res.preview.error || '编译未通过';
        engineStatus = {
          phase: 'error',
          message: '存在语法或编译错误',
        };
      }
    } catch (err) {
      if (seq !== compileSeq) return;
      compileError = err instanceof Error ? err.message : String(err);
      engineStatus = {
        phase: 'error',
        message: '编译异常',
      };
    }
  }

  async function refreshXRay() {
    if (!client) return;
    xrayLoading = true;
    xrayError = null;
    try {
      const wsSnap = {
        mainPath: mainFilePath,
        files,
        inputs: sysInputs,
      };
      openXmlPkg = await client.inspectOpenXml(wsSnap);
      if (irJson !== null) {
        const irRes = await client.inspectIr(wsSnap);
        irJson = irRes.irJson;
      }
    } catch (err) {
      xrayError = err instanceof Error ? err.message : String(err);
    } finally {
      xrayLoading = false;
    }
  }

  async function loadIrJson() {
    if (!client) return;
    try {
      const res = await client.inspectIr({
        mainPath: mainFilePath,
        files,
        inputs: sysInputs,
      });
      irJson = res.irJson;
    } catch (err) {
      xrayError = err instanceof Error ? err.message : String(err);
    }
  }

  function selectPreset(preset: StudioPreset) {
    activePresetId = preset.id;
    files = presetToVirtualFiles(preset);
    mainFilePath = preset.mainPath;
    activeFilePath = preset.mainPath;
    sysInputs = { ...(preset.inputs ?? {}) };
    compileError = null;
    irJson = null;
    scheduleCompile(20);
  }

  function handleEditorChange(nextText: string) {
    if (!activeFile || activeFile.text === null) return;
    const targetPath = activeFile.path;
    files = files.map((f) =>
      f.path === targetPath ? makeTextVirtualFile(targetPath, nextText) : f,
    );
    scheduleCompile(160);
  }

  function handleCreateFile(rawPath: string) {
    const clean = rawPath.replace(/^\/+/, '');
    if (!clean) return;
    if (files.some((f) => f.path === clean)) {
      activeFilePath = clean;
      return;
    }
    const starter = clean.endsWith('.typ')
      ? `// ${clean}\n`
      : '';
    files = [...files, makeTextVirtualFile(clean, starter)];
    activeFilePath = clean;
    activePresetId = 'custom';
    scheduleCompile(50);
  }

  function handleDeleteFile(path: string) {
    if (path === mainFilePath || files.length <= 1) return;
    files = files.filter((f) => f.path !== path);
    if (activeFilePath === path) {
      activeFilePath = mainFilePath;
    }
    scheduleCompile(40);
  }

  function handleSetMainFile(path: string) {
    mainFilePath = path;
    activeFilePath = path;
    scheduleCompile(20);
  }

  function handleUpdateInputs(next: Record<string, string>) {
    sysInputs = next;
    scheduleCompile(120);
  }

  async function handleExportDocx() {
    if (!client || isExporting) return;
    isExporting = true;
    const prevStatus = engineStatus;
    engineStatus = {
      phase: 'exporting',
      message: '正在生成 .docx…',
    };

    try {
      const res = await client.exportDocx({
        mainPath: mainFilePath,
        files,
        inputs: sysInputs,
      });
      report = res.report;

      const baseName =
        mainFilePath.replace(/\.typ$/i, '') ||
        documentTitle?.replace(/[\\/:*?"<>|]+/g, '_') ||
        'document';
      const outName = `${baseName}.docx`;

      const blob = new Blob([res.docxBytes as BlobPart], {
        type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = outName;
      document.body.appendChild(a);
      a.click();
      a.remove();
      setTimeout(() => URL.revokeObjectURL(url), 5000);

      engineStatus = {
        phase: 'ready',
        message: '就绪',
        elapsedMs: res.elapsedMs,
      };
      showToast(
        `已导出 ${outName} · ${(res.docxBytes.length / 1024).toFixed(1)} KB (${res.elapsedMs}ms)`,
      );
    } catch (err) {
      engineStatus = prevStatus;
      const msg = err instanceof Error ? err.message : String(err);
      compileError = msg;
      showToast(`导出失败：${msg}`);
    } finally {
      isExporting = false;
    }
  }

  async function handleSyncLocalFonts() {
    if (!client || !supportsLocalFontAccess) return;
    try {
      engineStatus = {
        phase: 'loading-font',
        message: '请求读取本机字体列表…',
      };
      const queryFn = (window as unknown as {
        queryLocalFonts: () => Promise<
          Array<{ family: string; fullName: string; blob: () => Promise<Blob> }>
        >;
      }).queryLocalFonts;
      const localList = await queryFn();

      const matched = localList.filter((item) => {
        const fam = item.family.trim().toLowerCase();
        return (
          PREFERRED_LOCAL_FAMILIES.has(fam) ||
          fam.includes('song') ||
          fam.includes('hei') ||
          fam.includes('kai') ||
          fam.includes('pingfang') ||
          fam.includes('yahei') ||
          fam.includes('noto') ||
          fam.includes('source han')
        );
      });

      const capped = matched.slice(0, 36);
      const payloads: Array<{
        name: string;
        buffer: ArrayBuffer;
        isSystemInstalled: boolean;
      }> = [];

      for (const f of capped) {
        try {
          const blob = await f.blob();
          const buffer = await blob.arrayBuffer();
          payloads.push({
            name: f.fullName || f.family,
            buffer,
            isSystemInstalled: true,
          });
        } catch {
          // Skip protected OS font faces
        }
      }

      if (payloads.length === 0) {
        showToast('未匹配到可读取的常用中英文字体');
        engineStatus = { phase: 'ready', message: '就绪' };
        return;
      }

      const res = await client.addFonts(payloads);
      fonts = res.fonts;
      showToast(`已同步 ${res.addedFaces} 个本机字体字面，正在重新校准行高…`);
      await runPreviewCompile();
    } catch (err) {
      engineStatus = { phase: 'ready', message: '就绪' };
      const msg = err instanceof Error ? err.message : String(err);
      showToast(`本机字体授权取消或受限：${msg}`);
    }
  }

  async function handleLoadCloudCjk() {
    if (!client) return;
    try {
      const res = await client.loadCloudCjk();
      fonts = res.fonts;
      showToast('云端思源宋体 (Noto Serif SC) 已就绪');
      await runPreviewCompile();
    } catch (err) {
      showToast(`加载云端字体失败：${err instanceof Error ? err.message : String(err)}`);
    }
  }

  async function handleUploadFontFiles(fileList: FileList) {
    if (!client) return;
    const payloads: Array<{
      name: string;
      buffer: ArrayBuffer;
      isSystemInstalled: boolean;
    }> = [];

    for (const file of Array.from(fileList)) {
      const buffer = await file.arrayBuffer();
      payloads.push({
        name: file.name,
        buffer,
        isSystemInstalled: true,
      });
    }
    if (payloads.length === 0) return;

    const res = await client.addFonts(payloads);
    fonts = res.fonts;
    showToast(`已载入 ${res.addedFaces} 个字体字面`);
    await runPreviewCompile();
  }

  async function ingestFiles(incoming: FileList | File[]) {
    const arr = Array.from(incoming);
    if (arr.length === 0) return;

    const fontItems = arr.filter((f) => /\.(ttf|otf|ttc)$/i.test(f.name));
    const projectItems = arr.filter((f) => !/\.(ttf|otf|ttc)$/i.test(f.name));

    if (fontItems.length > 0 && client) {
      const payloads = await Promise.all(
        fontItems.map(async (f) => ({
          name: f.name,
          buffer: await f.arrayBuffer(),
          isSystemInstalled: true,
        })),
      );
      const res = await client.addFonts(payloads);
      fonts = res.fonts;
      showToast(`已注册 ${res.addedFaces} 个自定义字体字面`);
    }

    if (projectItems.length === 0) {
      if (fontItems.length > 0) await runPreviewCompile();
      return;
    }

    // Single .zip archive -> unpack whole virtual workspace!
    if (projectItems.length === 1 && /\.zip$/i.test(projectItems[0].name)) {
      const zipBuf = await projectItems[0].arrayBuffer();
      const zip = await JSZip.loadAsync(zipBuf);
      const unpacked: VirtualFile[] = [];

      const entries = Object.values(zip.files).filter(
        (e) => !e.dir && !e.name.includes('__MACOSX') && !e.name.endsWith('.DS_Store'),
      );

      // Strip shared single top-level folder if present
      const firstSegs = new Set(
        entries.map((e) => e.name.split('/')[0]).filter(Boolean),
      );
      const stripPrefix =
        firstSegs.size === 1 && entries.every((e) => e.name.includes('/'))
          ? `${Array.from(firstSegs)[0]}/`
          : '';

      for (const entry of entries) {
        const relPath = stripPrefix && entry.name.startsWith(stripPrefix)
          ? entry.name.slice(stripPrefix.length)
          : entry.name;
        if (!relPath) continue;
        const bytes = await entry.async('uint8array');
        const text = isTextFilePath(relPath) ? decoder.decode(bytes) : null;
        unpacked.push({ path: relPath, bytes, text });
      }

      if (unpacked.length > 0) {
        const mainCandidate =
          unpacked.find((f) => f.path === 'main.typ') ??
          unpacked.find((f) => !f.path.includes('/') && f.path.endsWith('.typ')) ??
          unpacked.find((f) => f.path.endsWith('.typ')) ??
          unpacked[0];
        files = unpacked;
        mainFilePath = mainCandidate.path;
        activeFilePath = mainCandidate.path;
        activePresetId = 'custom';
        showToast(`已解包工程压缩包（共 ${unpacked.length} 个文件）`);
        scheduleCompile(20);
      }
      return;
    }

    // Regular project files (.typ, .bib, .png, .svg, etc.)
    const nextMap = new Map(files.map((f) => [f.path, f]));
    let uploadedTypPath: string | null = null;

    for (const item of projectItems) {
      const bytes = new Uint8Array(await item.arrayBuffer());
      const path = item.name.replace(/^\/+/, '');
      const text = isTextFilePath(path) ? decoder.decode(bytes) : null;
      nextMap.set(path, { path, bytes, text });
      if (path.endsWith('.typ')) {
        uploadedTypPath = path;
      }
    }

    files = Array.from(nextMap.values());
    if (uploadedTypPath) {
      activeFilePath = uploadedTypPath;
      if (projectItems.length === 1) {
        mainFilePath = uploadedTypPath;
        activePresetId = 'custom';
      }
    }
    showToast(`已导入 ${projectItems.length} 个文件至工作区`);
    scheduleCompile(30);
  }

  function changeStageMode(nextMode: StageMode, filterTag = '') {
    stageMode = nextMode;
    xrayFilterTag = filterTag;
    if (nextMode === 'xray') {
      void refreshXRay();
    }
  }

  function handleSplitPointerDown(e: PointerEvent) {
    e.preventDefault();
    isDraggingSplit = true;
    const onMove = (ev: PointerEvent) => {
      const sidebarWidth = sidebarOpen ? 236 : 0;
      const avail = window.innerWidth - sidebarWidth;
      if (avail <= 320) return;
      const rel = (ev.clientX - sidebarWidth) / avail;
      splitRatio = Math.min(0.7, Math.max(0.28, rel));
    };
    const onUp = () => {
      isDraggingSplit = false;
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
    };
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
  }

  function handleGlobalKeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'b') {
      e.preventDefault();
      sidebarOpen = !sidebarOpen;
    } else if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'e') {
      e.preventDefault();
      void handleExportDocx();
    }
  }

  onMount(() => {
    try {
      const saved = localStorage.getItem('typst2word-theme');
      if (saved === 'dark' || saved === 'light') {
        applyTheme(saved);
      } else if (window.matchMedia?.('(prefers-color-scheme: dark)').matches) {
        applyTheme('dark');
      } else {
        applyTheme('light');
      }
    } catch {
      applyTheme('light');
    }

    if (window.innerWidth < 920) {
      sidebarOpen = false;
    }

    client = new TypstWordWorkerClient({
      onBooted: ({ fonts: bootFonts, packages }) => {
        fonts = bootFonts;
        loadedPackages = packages;
      },
      onStatus: (phase, message) => {
        engineStatus = { phase, message };
      },
      onFontsUpdated: (updatedFonts, autoRecompile) => {
        fonts = updatedFonts;
        if (autoRecompile) {
          void runPreviewCompile();
        }
      },
      onPackagesUpdated: (packages) => {
        loadedPackages = packages;
      },
    });

    void runPreviewCompile();
  });

  onDestroy(() => {
    if (debounceTimer) clearTimeout(debounceTimer);
    if (toastTimer) clearTimeout(toastTimer);
    client?.destroy();
  });
</script>

<svelte:window
  onkeydown={handleGlobalKeydown}
  ondragover={(e) => {
    e.preventDefault();
    isDragOverWindow = true;
  }}
  ondragleave={(e) => {
    if (e.relatedTarget === null) {
      isDragOverWindow = false;
    }
  }}
  ondrop={(e) => {
    e.preventDefault();
    isDragOverWindow = false;
    if (e.dataTransfer?.files && e.dataTransfer.files.length > 0) {
      void ingestFiles(e.dataTransfer.files);
    }
  }}
/>

<div class="app-frame" class:resizing={isDraggingSplit}>
  <Toolbar
    {sidebarOpen}
    {activePresetId}
    presets={STUDIO_PRESETS}
    {stageMode}
    {engineStatus}
    {report}
    {fonts}
    {theme}
    {isExporting}
    {supportsLocalFontAccess}
    onToggleSidebar={() => {
      sidebarOpen = !sidebarOpen;
    }}
    onSelectPreset={selectPreset}
    onChangeStageMode={(m) => changeStageMode(m)}
    onSyncLocalFonts={handleSyncLocalFonts}
    onLoadCloudCjk={handleLoadCloudCjk}
    onUploadFontFiles={handleUploadFontFiles}
    onToggleTheme={() => applyTheme(theme === 'light' ? 'dark' : 'light')}
    onExportDocx={handleExportDocx}
  />

  <!-- Mobile editor/stage switcher -->
  <div class="mobile-pane-bar">
    <button
      type="button"
      class="mobile-tab"
      class:active={mobilePane === 'editor'}
      onclick={() => {
        mobilePane = 'editor';
      }}
    >
      Typst 源码
    </button>
    <button
      type="button"
      class="mobile-tab"
      class:active={mobilePane === 'stage'}
      onclick={() => {
        mobilePane = 'stage';
      }}
    >
      预览与体检
    </button>
  </div>

  <main class="workspace-body">
    {#if sidebarOpen}
      <WorkspaceSidebar
        {files}
        {activeFilePath}
        {mainFilePath}
        inputs={sysInputs}
        {loadedPackages}
        onSelectFile={(p) => {
          activeFilePath = p;
        }}
        onSetMainFile={handleSetMainFile}
        onCreateFile={handleCreateFile}
        onDeleteFile={handleDeleteFile}
        onUploadProjectFiles={(fl) => void ingestFiles(fl)}
        onUpdateInputs={handleUpdateInputs}
      />
    {/if}

    <div class="split-workbench" style={`--left-width: ${(splitRatio * 100).toFixed(2)}%;`}>
      <section class="pane-editor" class:mobile-hidden={mobilePane !== 'editor'}>
        {#if activeFile}
          {#if activeFile.text !== null}
            <TypstEditor
              value={activeFile.text}
              filePath={activeFile.path}
              readOnly={false}
              {theme}
              {compileError}
              onChange={handleEditorChange}
              onExportShortcut={() => void handleExportDocx()}
            />
          {:else}
            <div class="binary-preview">
              <div class="binary-card">
                <div class="binary-title mono">{activeFile.path}</div>
                <div class="binary-sub tabular">
                  二进制资源文件 ({(activeFile.bytes.length / 1024).toFixed(1)} KB) · 已挂载于虚拟工作区，可在 Typst 中通过
                  <code>#image("{activeFile.path}")</code> 直接引用。
                </div>
              </div>
            </div>
          {/if}
        {/if}
      </section>

      <!-- Resizable Apple hairline divider -->
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <div
        class="split-handle"
        role="separator"
        aria-orientation="vertical"
        title="拖拽调整左右分栏比例（双击恢复均衡宽度）"
        onpointerdown={handleSplitPointerDown}
        ondblclick={() => {
          splitRatio = 0.47;
        }}
      >
        <span class="handle-pill"></span>
      </div>

      <section class="pane-stage" class:mobile-hidden={mobilePane !== 'stage'}>
        {#if stageMode === 'preview'}
          <PageCanvas
            {pagesSvg}
            {report}
            {engineStatus}
            hasStaleError={compileError !== null}
            onJumpMode={(m) => changeStageMode(m)}
          />
        {:else if stageMode === 'telemetry'}
          <TelemetryView
            {report}
            {compilerWarnings}
            {fonts}
            {supportsLocalFontAccess}
            onSyncLocalFonts={handleSyncLocalFonts}
            onOpenXRay={(tag) => changeStageMode('xray', tag ?? '')}
            onExportDocx={handleExportDocx}
            onBackPreview={(m) => changeStageMode(m)}
          />
        {:else}
          <XRayInspector
            {openXmlPkg}
            {irJson}
            loading={xrayLoading}
            error={xrayError}
            initialFilterTag={xrayFilterTag}
            onRefresh={refreshXRay}
            onLoadIr={loadIrJson}
          />
        {/if}
      </section>
    </div>
  </main>

  {#if isDragOverWindow}
    <div class="drop-overlay" aria-hidden="true">
      <div class="drop-sheet">
        <div class="drop-title">松开以导入到 typst2word 工作区</div>
        <div class="drop-sub">
          支持 <code>.typ</code> 源码、<code>.bib</code> 文献、图片资源、<code>.ttf/.otf</code>
          字体或整个 <code>.zip</code> 工程包
        </div>
      </div>
    </div>
  {/if}

  {#if toastMessage}
    <div class="apple-toast tabular" role="status" aria-live="polite">
      {toastMessage}
    </div>
  {/if}
</div>

<style>
  .app-frame {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: var(--bg-app);
  }

  .app-frame.resizing {
    cursor: col-resize;
    user-select: none;
  }

  .mobile-pane-bar {
    display: none;
    padding: 6px 12px;
    gap: 6px;
    background: var(--bg-sidebar);
    border-bottom: 1px solid var(--border-hairline);
  }

  .mobile-tab {
    flex: 1;
    height: 28px;
    border-radius: 7px;
    font-size: 12px;
    font-weight: 500;
    background: var(--bg-subtle);
    color: var(--text-secondary);
  }

  .mobile-tab.active {
    background: var(--accent-blue);
    color: #fff;
    font-weight: 600;
  }

  .workspace-body {
    display: flex;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .split-workbench {
    display: flex;
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .pane-editor {
    width: var(--left-width, 47%);
    min-width: 260px;
    height: 100%;
    flex-shrink: 0;
    overflow: hidden;
  }

  .split-handle {
    position: relative;
    width: 7px;
    margin: 0 -3px;
    z-index: 20;
    cursor: col-resize;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .split-handle::before {
    content: '';
    position: absolute;
    top: 0;
    bottom: 0;
    left: 3px;
    width: 1px;
    background: var(--border-hairline);
    transition: background 0.15s ease;
  }

  .split-handle:hover::before,
  .app-frame.resizing .split-handle::before {
    background: var(--accent-blue);
    width: 2px;
  }

  .handle-pill {
    width: 3px;
    height: 28px;
    border-radius: 999px;
    background: var(--border-strong);
    opacity: 0;
    transition: opacity 0.15s ease;
  }

  .split-handle:hover .handle-pill {
    opacity: 1;
  }

  .pane-stage {
    flex: 1;
    min-width: 280px;
    height: 100%;
    overflow: hidden;
  }

  .binary-preview {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    padding: 24px;
    background: var(--bg-surface);
  }

  .binary-card {
    max-width: 360px;
    padding: 20px;
    border-radius: 14px;
    background: var(--bg-sidebar);
    border: 1px solid var(--border-hairline);
    text-align: center;
  }

  .binary-title {
    font-size: 13px;
    font-weight: 600;
    margin-bottom: 8px;
  }

  .binary-sub {
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-secondary);
  }

  .drop-overlay {
    position: fixed;
    inset: 12px;
    z-index: 90;
    border-radius: 18px;
    border: 2px dashed var(--accent-blue);
    background: rgba(0, 113, 227, 0.08);
    backdrop-filter: blur(8px);
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: none;
  }

  .drop-sheet {
    padding: 24px 32px;
    border-radius: 16px;
    background: var(--bg-popover);
    box-shadow: var(--shadow-popover);
    text-align: center;
  }

  .drop-title {
    font-size: 16px;
    font-weight: 650;
    color: var(--text-primary);
    margin-bottom: 6px;
  }

  .drop-sub {
    font-size: 12.5px;
    color: var(--text-secondary);
  }

  .apple-toast {
    position: fixed;
    left: 50%;
    bottom: 22px;
    transform: translateX(-50%);
    z-index: 100;
    padding: 8px 16px;
    border-radius: 999px;
    background: var(--bg-popover);
    backdrop-filter: saturate(180%) blur(20px);
    -webkit-backdrop-filter: saturate(180%) blur(20px);
    border: 1px solid var(--border-strong);
    box-shadow: var(--shadow-popover);
    font-size: 12.5px;
    font-weight: 500;
    color: var(--text-primary);
    pointer-events: none;
  }

  @media (max-width: 780px) {
    .mobile-pane-bar {
      display: flex;
    }
    .split-handle {
      display: none;
    }
    .pane-editor,
    .pane-stage {
      width: 100% !important;
      min-width: 0;
    }
    .mobile-hidden {
      display: none;
    }
  }
</style>
