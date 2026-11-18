<script lang="ts">
  import type { ConversionReport, EngineStatus, StageMode } from '../types';

  interface Props {
    pagesSvg: string[];
    report: ConversionReport | null;
    engineStatus: EngineStatus;
    hasStaleError: boolean;
    onJumpMode: (mode: StageMode) => void;
  }

  let { pagesSvg, report, engineStatus, hasStaleError, onJumpMode }: Props = $props();

  let zoomMode = $state<'fit' | number>('fit');
  const zoomPresets = [0.75, 1, 1.25, 1.5];

  function stepZoom(delta: number) {
    const current = zoomMode === 'fit' ? 1 : zoomMode;
    const next = Math.min(1.75, Math.max(0.5, Math.round((current + delta) * 100) / 100));
    zoomMode = next;
  }
</script>

<div class="canvas-root">
  <!-- Subtle floating sub-header with live Word construct summary & zoom -->
  <div class="canvas-subheader">
    <div class="construct-strip tabular">
      {#if report}
        <button
          type="button"
          class="telemetry-chip"
          title="点击查看完整转换体检报告"
          onclick={() => onJumpMode('telemetry')}
        >
          <span>{report.pages_count} 页</span>
          {#if report.headings_count > 0}
            <span class="sep">·</span>
            <span>{report.headings_count} 标题</span>
          {/if}
          {#if report.equations_count > 0}
            <span class="sep">·</span>
            <span class="accent">{report.equations_count} OMML 公式</span>
          {/if}
          {#if report.tables_count > 0}
            <span class="sep">·</span>
            <span>{report.tables_count} 表格</span>
          {/if}
          {#if report.shapes_count + report.cetz_canvases_count > 0}
            <span class="sep">·</span>
            <span class="accent">
              {report.shapes_count + report.cetz_canvases_count} 矢量图
            </span>
          {/if}
        </button>
        <button
          type="button"
          class="xray-link"
          onclick={() => onJumpMode('xray')}
        >
          检视 OpenXML →
        </button>
      {:else}
        <span class="muted-hint">Typst 0.15 物理排版基准视图</span>
      {/if}
    </div>

    <div class="zoom-controls tabular">
      <button
        type="button"
        class="zoom-step"
        aria-label="缩小"
        onclick={() => stepZoom(-0.15)}
      >
        −
      </button>
      <select
        class="zoom-select"
        aria-label="缩放比例"
        value={zoomMode === 'fit' ? 'fit' : String(zoomMode)}
        onchange={(e) => {
          const val = e.currentTarget.value;
          zoomMode = val === 'fit' ? 'fit' : Number(val);
        }}
      >
        <option value="fit">自适应宽</option>
        {#each zoomPresets as z (z)}
          <option value={String(z)}>{Math.round(z * 100)}%</option>
        {/each}
      </select>
      <button
        type="button"
        class="zoom-step"
        aria-label="放大"
        onclick={() => stepZoom(0.15)}
      >
        +
      </button>
    </div>
  </div>

  {#if hasStaleError && pagesSvg.length > 0}
    <div class="stale-banner">
      当前代码存在语法错误，画布暂留最近一次成功编译的版面。
    </div>
  {/if}

  <div class="pages-scroll">
    {#if pagesSvg.length > 0}
      <div
        class="pages-stack"
        class:fit-width={zoomMode === 'fit'}
        class:dimmed={hasStaleError}
        style={zoomMode !== 'fit' ? `--page-scale: ${zoomMode};` : ''}
      >
        {#each pagesSvg as svgMarkup, idx (idx)}
          <article class="paper-sheet" aria-label={`第 ${idx + 1} 页`}>
            <div class="svg-mount">
              {@html svgMarkup}
            </div>
            <div class="page-caption tabular">
              第 {idx + 1} / {pagesSvg.length} 页
            </div>
          </article>
        {/each}
      </div>
    {:else if engineStatus.phase === 'booting' || engineStatus.phase === 'compiling' || engineStatus.phase === 'fetching-package'}
      <div class="boot-card">
        <div class="spinner" aria-hidden="true"></div>
        <div class="boot-title">{engineStatus.message}</div>
        <div class="boot-sub">
          首次加载约需下载 WebAssembly 编译器与云端字体，完成后将持久缓存于浏览器本地。
        </div>
      </div>
    {:else}
      <div class="boot-card">
        <div class="boot-title">等待有效排版输出</div>
        <div class="boot-sub">请修正左侧编辑器中的语法错误后自动刷新预览。</div>
      </div>
    {/if}
  </div>
</div>

<style>
  .canvas-root {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    background: var(--bg-canvas);
    overflow: hidden;
  }

  .canvas-subheader {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    height: 36px;
    padding: 0 14px;
    background: var(--bg-sidebar);
    border-bottom: 1px solid var(--border-hairline);
    flex-shrink: 0;
  }

  .construct-strip {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    overflow: hidden;
  }

  .telemetry-chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 3px 9px;
    border-radius: 999px;
    background: var(--bg-subtle);
    font-size: 11.5px;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    transition: background 0.14s ease;
  }

  .telemetry-chip:hover {
    background: var(--bg-segmented);
    color: var(--text-primary);
  }

  .telemetry-chip .sep {
    color: var(--text-tertiary);
  }

  .telemetry-chip .accent {
    color: var(--accent-blue);
    font-weight: 600;
  }

  .xray-link {
    font-size: 11.5px;
    font-weight: 500;
    color: var(--accent-blue);
    padding: 2px 6px;
    border-radius: 5px;
    white-space: nowrap;
  }

  .xray-link:hover {
    background: var(--accent-blue-soft);
  }

  .muted-hint {
    font-size: 11.5px;
    color: var(--text-tertiary);
  }

  .zoom-controls {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }

  .zoom-step {
    height: 24px;
    padding: 0 7px;
    border-radius: 6px;
    font-size: 11.5px;
    color: var(--text-secondary);
    background: var(--bg-subtle);
  }

  .zoom-step:hover {
    background: var(--bg-segmented);
    color: var(--text-primary);
  }

  .zoom-select {
    height: 24px;
    padding: 0 4px;
    border-radius: 6px;
    border: 1px solid var(--border-hairline);
    background: var(--bg-surface);
    font-size: 11.5px;
    color: var(--text-primary);
  }

  .stale-banner {
    padding: 5px 14px;
    background: var(--status-amber-bg);
    color: var(--status-amber);
    font-size: 11.5px;
    font-weight: 500;
    border-bottom: 1px solid var(--border-hairline);
    flex-shrink: 0;
  }

  .pages-scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 24px 24px 42px 24px;
    display: flex;
    flex-direction: column;
    align-items: center;
  }

  .pages-stack {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 26px;
    width: 100%;
    transition: opacity 0.2s ease;
  }

  .pages-stack.dimmed {
    opacity: 0.76;
  }

  .paper-sheet {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    max-width: 100%;
  }

  .svg-mount {
    background: #ffffff;
    border-radius: 3px;
    box-shadow: var(--shadow-paper);
    overflow: hidden;
    line-height: 0;
  }

  .pages-stack.fit-width .paper-sheet {
    width: min(100%, 760px);
  }

  .pages-stack.fit-width .svg-mount {
    width: 100%;
  }

  .pages-stack.fit-width .svg-mount :global(svg) {
    width: 100% !important;
    height: auto !important;
    display: block;
  }

  .pages-stack:not(.fit-width) .svg-mount :global(svg) {
    width: calc(595pt * var(--page-scale, 1));
    height: auto !important;
    display: block;
  }

  .page-caption {
    font-size: 11px;
    color: var(--text-tertiary);
    user-select: none;
  }

  .boot-card {
    margin: auto;
    max-width: 380px;
    text-align: center;
    padding: 28px 24px;
    border-radius: 16px;
    background: var(--bg-surface);
    border: 1px solid var(--border-hairline);
    box-shadow: var(--shadow-card);
  }

  .spinner {
    width: 24px;
    height: 24px;
    margin: 0 auto 12px auto;
    border-radius: 50%;
    border: 2.5px solid var(--border-strong);
    border-top-color: var(--accent-blue);
    animation: spin 0.75s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .boot-title {
    font-size: 13.5px;
    font-weight: 600;
    color: var(--text-primary);
    margin-bottom: 6px;
  }

  .boot-sub {
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-secondary);
  }
</style>
