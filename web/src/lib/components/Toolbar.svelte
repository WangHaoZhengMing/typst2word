<script lang="ts">
  import type {
    ConversionReport,
    EngineStatus,
    FontFamilyDto,
    StageMode,
    StudioPreset,
  } from '../types';

  interface Props {
    sidebarOpen: boolean;
    activePresetId: string;
    presets: StudioPreset[];
    stageMode: StageMode;
    engineStatus: EngineStatus;
    report: ConversionReport | null;
    fonts: FontFamilyDto[];
    theme: 'light' | 'dark';
    isExporting: boolean;
    supportsLocalFontAccess: boolean;
    onToggleSidebar: () => void;
    onSelectPreset: (preset: StudioPreset) => void;
    onChangeStageMode: (mode: StageMode) => void;
    onSyncLocalFonts: () => void;
    onLoadCloudCjk: () => void;
    onUploadFontFiles: (files: FileList) => void;
    onToggleTheme: () => void;
    onExportDocx: () => void;
  }

  let {
    sidebarOpen,
    activePresetId,
    presets,
    stageMode,
    engineStatus,
    report,
    fonts,
    theme,
    isExporting,
    supportsLocalFontAccess,
    onToggleSidebar,
    onSelectPreset,
    onChangeStageMode,
    onSyncLocalFonts,
    onLoadCloudCjk,
    onUploadFontFiles,
    onToggleTheme,
    onExportDocx,
  }: Props = $props();

  let openMenu = $state<'presets' | 'fonts' | null>(null);
  let fontInputEl: HTMLInputElement | undefined = $state();

  const activePreset = $derived(presets.find((p) => p.id === activePresetId));
  const systemFontsCount = $derived(fonts.filter((f) => f.is_system_installed).length);
  const warningCount = $derived(
    (report?.font_fallback_warnings?.length ?? 0) +
      (report?.unsupported_elements?.length ?? 0),
  );

  function toggleMenu(target: 'presets' | 'fonts') {
    openMenu = openMenu === target ? null : target;
  }

  function closeMenus() {
    openMenu = null;
  }

  function handleWindowKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && openMenu) {
      openMenu = null;
    }
  }
</script>

<svelte:window onkeydown={handleWindowKeydown} />

{#if openMenu}
  <button
    type="button"
    class="menu-backdrop"
    aria-label="关闭弹出面板"
    onclick={closeMenus}
  ></button>
{/if}

<header class="studio-toolbar">
  <div class="toolbar-left">
    <button
      type="button"
      class="icon-btn"
      class:active={sidebarOpen}
      title={sidebarOpen ? '收起工作区侧栏 (⌘B)' : '展开工作区侧栏 (⌘B)'}
      aria-label="切换侧边栏"
      onclick={onToggleSidebar}
    >
      <svg viewBox="0 0 18 18" fill="none" aria-hidden="true">
        <rect
          x="2"
          y="3"
          width="14"
          height="12"
          rx="2.4"
          stroke="currentColor"
          stroke-width="1.35"
        />
        <path d="M6.6 3.2v11.6" stroke="currentColor" stroke-width="1.35" />
      </svg>
    </button>

    <div class="brand">
      <div class="brand-mark" aria-hidden="true">
        <span>T</span>
        <span class="arrow">→</span>
        <span>W</span>
      </div>
      <div class="brand-text">
        <span class="brand-name">typst2word</span>
        <span class="brand-tag">Studio</span>
      </div>
    </div>

    <div class="toolbar-divider"></div>

    <div class="popover-anchor">
      <button
        type="button"
        class="preset-trigger"
        class:open={openMenu === 'presets'}
        onclick={() => toggleMenu('presets')}
      >
        <span class="preset-label">{activePreset ? activePreset.title : '自定义工程'}</span>
        <svg class="chevron" viewBox="0 0 12 12" fill="none" aria-hidden="true">
          <path
            d="M3 4.5L6 7.5L9 4.5"
            stroke="currentColor"
            stroke-width="1.4"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
      </button>

      {#if openMenu === 'presets'}
        <div class="popover-panel presets-popover" role="menu">
          <div class="popover-header">内置精选模板</div>
          {#each presets as preset (preset.id)}
            <button
              type="button"
              class="preset-item"
              class:selected={preset.id === activePresetId}
              onclick={() => {
                onSelectPreset(preset);
                closeMenus();
              }}
            >
              <div class="preset-item-top">
                <span class="preset-item-title">{preset.title}</span>
                <span class="preset-badge">{preset.badge}</span>
              </div>
              <div class="preset-item-sub">{preset.subtitle}</div>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  <div class="toolbar-center">
    <div class="segmented-control" role="tablist" aria-label="右侧检视模式">
      <button
        type="button"
        role="tab"
        aria-selected={stageMode === 'preview'}
        class="seg-btn"
        class:active={stageMode === 'preview'}
        onclick={() => onChangeStageMode('preview')}
      >
        排版画布
      </button>
      <button
        type="button"
        role="tab"
        aria-selected={stageMode === 'telemetry'}
        class="seg-btn"
        class:active={stageMode === 'telemetry'}
        onclick={() => onChangeStageMode('telemetry')}
      >
        <span>转换体检</span>
        {#if warningCount > 0}
          <span class="warn-pip tabular">{warningCount}</span>
        {:else if report && report.equations_count + report.tables_count + report.shapes_count + report.cetz_canvases_count > 0}
          <span class="ok-pip tabular">
            {report.equations_count +
              report.tables_count +
              report.shapes_count +
              report.cetz_canvases_count}
          </span>
        {/if}
      </button>
      <button
        type="button"
        role="tab"
        aria-selected={stageMode === 'xray'}
        class="seg-btn"
        class:active={stageMode === 'xray'}
        onclick={() => onChangeStageMode('xray')}
      >
        OpenXML 透视
      </button>
    </div>
  </div>

  <div class="toolbar-right">
    <div
      class="status-pill tabular"
      class:busy={
        engineStatus.phase === 'booting' ||
        engineStatus.phase === 'compiling' ||
        engineStatus.phase === 'fetching-package' ||
        engineStatus.phase === 'loading-font' ||
        engineStatus.phase === 'exporting'
      }
      class:error={engineStatus.phase === 'error'}
      title={engineStatus.message}
    >
      <span class="status-dot"></span>
      <span class="status-text">
        {#if engineStatus.phase === 'ready'}
          就绪{engineStatus.elapsedMs !== undefined ? ` · ${engineStatus.elapsedMs}ms` : ''}
        {:else}
          {engineStatus.message}
        {/if}
      </span>
    </div>

    <div class="popover-anchor">
      <button
        type="button"
        class="pill-secondary tabular"
        class:open={openMenu === 'fonts'}
        onclick={() => toggleMenu('fonts')}
        title="管理排版字体与本机 Word 字体度量同步"
      >
        <svg class="btn-glyph" viewBox="0 0 16 16" fill="none" aria-hidden="true">
          <path
            d="M2.5 12.5L6.2 3.5h1.6l3.7 9M4 9.2h6M11.2 6.8h2.6M12.5 5.5v7"
            stroke="currentColor"
            stroke-width="1.35"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
        <span>字体</span>
        <span class="count-chip">{fonts.length}</span>
      </button>

      {#if openMenu === 'fonts'}
        <div class="popover-panel fonts-popover">
          <div class="popover-header">字体引擎与 Word 行盒校准</div>
          <p class="popover-desc">
            <code>typst2word</code> 直接解析字体二进制 <code>OS/2</code> 表补偿中文 <code>1.3×</code>
            行高扩张。同步本机字体后，WASM 编译将使用与桌面端 Word 完全一致的字形度量。
          </p>

          <div class="font-actions">
            {#if supportsLocalFontAccess}
              <button
                type="button"
                class="font-action-primary"
                onclick={() => {
                  onSyncLocalFonts();
                  closeMenus();
                }}
              >
                <span>同步本机常用中英文字体</span>
                <span class="action-tag">推荐</span>
              </button>
            {/if}

            <button
              type="button"
              class="font-action-btn"
              onclick={() => {
                onLoadCloudCjk();
                closeMenus();
              }}
            >
              加载云端思源宋体 (Noto Serif SC)
            </button>

            <button
              type="button"
              class="font-action-btn"
              onclick={() => fontInputEl?.click()}
            >
              上传本地字体文件 (.ttf / .otf / .ttc)…
            </button>
            <input
              bind:this={fontInputEl}
              type="file"
              accept=".ttf,.otf,.ttc"
              multiple
              hidden
              onchange={(e) => {
                const input = e.currentTarget;
                if (input.files && input.files.length > 0) {
                  onUploadFontFiles(input.files);
                  input.value = '';
                  closeMenus();
                }
              }}
            />
          </div>

          <div class="font-list-header tabular">
            <span>已注册字体族 ({fonts.length})</span>
            {#if systemFontsCount > 0}
              <span class="sys-tag">本机同步 {systemFontsCount}</span>
            {/if}
          </div>
          <div class="font-list">
            {#each fonts as f (f.family)}
              <div class="font-row">
                <span class="font-family">{f.family}</span>
                <span class="font-origin" class:sys={f.is_system_installed}>
                  {f.is_system_installed ? '本机安装' : '内置 / 云端'}
                </span>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </div>

    <button
      type="button"
      class="icon-btn"
      title={theme === 'light' ? '切换深色外观' : '切换浅色外观'}
      aria-label="切换深浅外观"
      onclick={onToggleTheme}
    >
      {#if theme === 'light'}
        <svg viewBox="0 0 18 18" fill="none" aria-hidden="true">
          <path
            d="M14.2 11.1A5.8 5.8 0 0 1 6.9 3.8a5.8 5.8 0 1 0 7.3 7.3Z"
            stroke="currentColor"
            stroke-width="1.4"
            stroke-linejoin="round"
          />
        </svg>
      {:else}
        <svg viewBox="0 0 18 18" fill="none" aria-hidden="true">
          <circle cx="9" cy="9" r="3.4" stroke="currentColor" stroke-width="1.4" />
          <path
            d="M9 2.2v1.7M9 14.1v1.7M2.2 9h1.7M14.1 9h1.7M4.2 4.2l1.2 1.2M12.6 12.6l1.2 1.2M13.8 4.2l-1.2 1.2M5.4 12.6l-1.2 1.2"
            stroke="currentColor"
            stroke-width="1.4"
            stroke-linecap="round"
          />
        </svg>
      {/if}
    </button>

    <a
      href="https://github.com/WangHaoZhengMing/typst2word"
      target="_blank"
      rel="noreferrer"
      class="icon-btn"
      title="查看 GitHub 仓库与桌面端 CLI"
      aria-label="GitHub 仓库"
    >
      <svg viewBox="0 0 18 18" fill="currentColor" aria-hidden="true">
        <path
          fill-rule="evenodd"
          clip-rule="evenodd"
          d="M9 1.8a7.2 7.2 0 0 0-2.28 14.03c.36.07.49-.15.49-.34v-1.2c-2 .43-2.42-.96-2.42-.96-.33-.83-.8-1.05-.8-1.05-.65-.45.05-.44.05-.44.72.05 1.1.74 1.1.74.64 1.1 1.68.78 2.09.6.06-.47.25-.78.45-.96-1.6-.18-3.28-.8-3.28-3.56 0-.79.28-1.43.74-1.94-.07-.18-.32-.91.07-1.9 0 0 .6-.19 1.98.74a6.86 6.86 0 0 1 3.6 0c1.38-.93 1.98-.74 1.98-.74.39.99.14 1.72.07 1.9.46.51.74 1.15.74 1.94 0 2.77-1.68 3.38-3.29 3.56.26.22.49.66.49 1.34v1.98c0 .19.13.41.5.34A7.2 7.2 0 0 0 9 1.8Z"
        />
      </svg>
    </a>

    <button
      type="button"
      class="export-cta"
      disabled={isExporting || engineStatus.phase === 'booting'}
      onclick={onExportDocx}
      title="编译并下载原生可编辑 Microsoft Word 文档 (⌘E / Ctrl+E)"
    >
      <svg class="cta-icon" viewBox="0 0 16 16" fill="none" aria-hidden="true">
        <path
          d="M8 2.5v7.2M5.2 7.2L8 10l2.8-2.8M3 12.8h10"
          stroke="currentColor"
          stroke-width="1.6"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
      <span>{isExporting ? '正在生成…' : '导出 .docx'}</span>
      <kbd class="cta-kbd">⌘E</kbd>
    </button>
  </div>
</header>

<style>
  .menu-backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: transparent;
    cursor: default;
  }

  .studio-toolbar {
    position: relative;
    z-index: 50;
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    gap: 12px;
    height: 52px;
    padding: 0 14px;
    background: var(--bg-toolbar);
    backdrop-filter: saturate(180%) blur(20px);
    -webkit-backdrop-filter: saturate(180%) blur(20px);
    border-bottom: 1px solid var(--border-hairline);
    flex-shrink: 0;
  }

  .toolbar-left {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .toolbar-center {
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .toolbar-right {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    min-width: 0;
  }

  .icon-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border-radius: 7px;
    color: var(--text-secondary);
    text-decoration: none;
    transition: background 0.15s ease, color 0.15s ease;
    flex-shrink: 0;
  }

  .icon-btn svg {
    width: 17px;
    height: 17px;
  }

  .icon-btn:hover,
  .icon-btn.active {
    background: var(--bg-subtle);
    color: var(--text-primary);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    user-select: none;
    flex-shrink: 0;
  }

  .brand-mark {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    padding: 3px 7px;
    border-radius: 7px;
    background: linear-gradient(145deg, #0a84ff, #0055d4);
    color: #ffffff;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: -0.02em;
    box-shadow: 0 1px 3px rgba(0, 113, 227, 0.28);
  }

  .brand-mark .arrow {
    opacity: 0.78;
    font-weight: 500;
  }

  .brand-text {
    display: flex;
    align-items: baseline;
    gap: 5px;
  }

  .brand-name {
    font-size: 14px;
    font-weight: 600;
    letter-spacing: -0.02em;
    color: var(--text-primary);
  }

  .brand-tag {
    font-size: 11px;
    font-weight: 500;
    color: var(--text-tertiary);
  }

  .toolbar-divider {
    width: 1px;
    height: 18px;
    background: var(--border-hairline);
    margin: 0 2px;
    flex-shrink: 0;
  }

  .popover-anchor {
    position: relative;
    min-width: 0;
  }

  .preset-trigger {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    max-width: 220px;
    height: 30px;
    padding: 0 10px;
    border-radius: 8px;
    background: var(--bg-subtle);
    color: var(--text-primary);
    font-size: 12.5px;
    font-weight: 500;
    transition: background 0.15s ease;
  }

  .preset-trigger:hover,
  .preset-trigger.open {
    background: var(--bg-segmented);
  }

  .preset-label {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .chevron {
    width: 12px;
    height: 12px;
    color: var(--text-tertiary);
    flex-shrink: 0;
  }

  .segmented-control {
    display: inline-flex;
    align-items: center;
    padding: 2.5px;
    border-radius: 9px;
    background: var(--bg-segmented);
    gap: 2px;
  }

  .seg-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 27px;
    padding: 0 12px;
    border-radius: 7px;
    font-size: 12.5px;
    font-weight: 500;
    color: var(--text-secondary);
    transition: background 0.16s ease, color 0.16s ease, box-shadow 0.16s ease;
    white-space: nowrap;
  }

  .seg-btn:hover {
    color: var(--text-primary);
  }

  .seg-btn.active {
    background: var(--bg-segmented-active);
    color: var(--text-primary);
    box-shadow: var(--shadow-segmented);
    font-weight: 600;
  }

  .ok-pip,
  .warn-pip {
    font-size: 10.5px;
    font-weight: 600;
    padding: 0.5px 5.5px;
    border-radius: 999px;
  }

  .ok-pip {
    background: var(--accent-blue-soft);
    color: var(--accent-blue);
  }

  .warn-pip {
    background: var(--status-amber-bg);
    color: var(--status-amber);
  }

  .status-pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    max-width: 175px;
    flex-shrink: 1;
    height: 28px;
    padding: 0 10px;
    border-radius: 999px;
    background: var(--bg-subtle);
    font-size: 11.5px;
    color: var(--text-secondary);
    white-space: nowrap;
  }

  .status-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--status-green);
    flex-shrink: 0;
  }

  .status-pill.busy .status-dot {
    background: var(--accent-blue);
    animation: pulse-dot 1s ease-in-out infinite;
  }

  .status-pill.error {
    background: var(--status-red-bg);
    color: var(--status-red);
  }

  .status-pill.error .status-dot {
    background: var(--status-red);
  }

  .status-text {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  @keyframes pulse-dot {
    0%,
    100% {
      opacity: 1;
      transform: scale(1);
    }
    50% {
      opacity: 0.4;
      transform: scale(0.82);
    }
  }

  .pill-secondary {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 30px;
    padding: 0 10px;
    border-radius: 8px;
    background: var(--bg-subtle);
    color: var(--text-primary);
    font-size: 12px;
    font-weight: 500;
    white-space: nowrap;
    transition: background 0.15s ease;
    flex-shrink: 0;
  }

  .pill-secondary:hover,
  .pill-secondary.open {
    background: var(--bg-segmented);
  }

  .btn-glyph {
    width: 14px;
    height: 14px;
    color: var(--text-secondary);
  }

  .count-chip {
    font-size: 10.5px;
    padding: 0.5px 5px;
    border-radius: 999px;
    background: var(--bg-segmented);
    color: var(--text-secondary);
  }

  .export-cta {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 32px;
    padding: 0 12px;
    border-radius: 999px;
    background: var(--accent-blue);
    color: var(--text-inverse);
    font-size: 12.5px;
    font-weight: 600;
    box-shadow: 0 1px 3px rgba(0, 113, 227, 0.32);
    transition: background 0.15s ease, transform 0.1s ease, opacity 0.15s ease;
    flex-shrink: 0;
  }

  .export-cta:hover:not(:disabled) {
    background: var(--accent-blue-hover);
  }

  .export-cta:active:not(:disabled) {
    background: var(--accent-blue-active);
    transform: scale(0.98);
  }

  .export-cta:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .cta-icon {
    width: 14px;
    height: 14px;
  }

  .cta-kbd {
    font-family: inherit;
    font-size: 10.5px;
    font-weight: 500;
    padding: 1px 5px;
    border-radius: 5px;
    background: rgba(255, 255, 255, 0.2);
  }

  /* Frosted Popover Sheets */
  .popover-panel {
    position: absolute;
    top: calc(100% + 8px);
    z-index: 60;
    border-radius: 13px;
    background: var(--bg-popover);
    backdrop-filter: saturate(190%) blur(24px);
    -webkit-backdrop-filter: saturate(190%) blur(24px);
    border: 1px solid var(--border-strong);
    box-shadow: var(--shadow-popover);
    padding: 8px;
  }

  .presets-popover {
    left: 0;
    width: 340px;
  }

  .fonts-popover {
    right: 0;
    width: 330px;
  }

  .popover-header {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--text-tertiary);
    padding: 4px 8px 6px 8px;
  }

  .popover-desc {
    margin: 0 8px 10px 8px;
    font-size: 11.5px;
    line-height: 1.5;
    color: var(--text-secondary);
  }

  .popover-desc code {
    font-size: 11px;
    padding: 1px 4px;
    border-radius: 4px;
    background: var(--bg-subtle);
  }

  .preset-item {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 3px;
    width: 100%;
    text-align: left;
    padding: 8px 10px;
    border-radius: 9px;
    transition: background 0.14s ease;
  }

  .preset-item:hover {
    background: var(--bg-subtle);
  }

  .preset-item.selected {
    background: var(--accent-blue-soft);
  }

  .preset-item-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .preset-item-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .preset-badge {
    font-size: 10.5px;
    font-weight: 600;
    padding: 1.5px 6.5px;
    border-radius: 999px;
    background: var(--bg-subtle);
    color: var(--accent-blue);
    flex-shrink: 0;
  }

  .preset-item-sub {
    font-size: 11.5px;
    color: var(--text-secondary);
  }

  .font-actions {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 0 6px 10px 6px;
    border-bottom: 1px solid var(--border-hairline);
  }

  .font-action-primary {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: 8px 10px;
    border-radius: 8px;
    background: var(--accent-blue);
    color: #ffffff;
    font-size: 12px;
    font-weight: 600;
  }

  .font-action-primary:hover {
    background: var(--accent-blue-hover);
  }

  .action-tag {
    font-size: 10px;
    padding: 1px 5px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.22);
  }

  .font-action-btn {
    width: 100%;
    text-align: left;
    padding: 7px 10px;
    border-radius: 8px;
    background: var(--bg-subtle);
    color: var(--text-primary);
    font-size: 12px;
    font-weight: 500;
    transition: background 0.14s ease;
  }

  .font-action-btn:hover {
    background: var(--bg-segmented);
  }

  .font-list-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 8px 4px 8px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-tertiary);
  }

  .sys-tag {
    color: var(--status-green);
  }

  .font-list {
    max-height: 165px;
    overflow-y: auto;
    padding: 0 4px;
  }

  .font-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 6px;
    border-radius: 6px;
    font-size: 11.5px;
  }

  .font-family {
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .font-origin {
    font-size: 10.5px;
    color: var(--text-tertiary);
    flex-shrink: 0;
  }

  .font-origin.sys {
    color: var(--status-green);
    font-weight: 500;
  }

  @media (max-width: 1040px) {
    .studio-toolbar {
      grid-template-columns: auto 1fr auto;
    }
    .brand-tag,
    .status-pill,
    .cta-kbd {
      display: none;
    }
    .preset-trigger {
      max-width: 150px;
    }
  }

  @media (max-width: 840px) {
    .brand-text,
    .toolbar-divider,
    .pill-secondary span {
      display: none;
    }
    .preset-trigger {
      max-width: 120px;
    }
  }
</style>
