<script lang="ts">
  import type { OpenXmlPackageDto } from '../types';

  interface Props {
    openXmlPkg: OpenXmlPackageDto | null;
    irJson: string | null;
    loading: boolean;
    error: string | null;
    initialFilterTag: string;
    onRefresh: () => void;
    onLoadIr: () => void;
  }

  let {
    openXmlPkg,
    irJson,
    loading,
    error,
    initialFilterTag,
    onRefresh,
    onLoadIr,
  }: Props = $props();

  let selectedTarget = $state<string>('word/document.xml');
  let searchFilter = $state<string>('');
  let copied = $state(false);

  $effect(() => {
    if (initialFilterTag) {
      selectedTarget = 'word/document.xml';
      searchFilter = initialFilterTag;
    }
  });

  const filterPresets = [
    { label: '全部源码', query: '' },
    { label: 'OMML 公式', query: '<m:oMath' },
    { label: 'DrawingML 图形', query: '<wps:wsp' },
    { label: 'CeTZ 组合图', query: '<wpg:wgp' },
    { label: '表格结构', query: '<w:tbl' },
    { label: '行距与字距补偿', query: '<w:spacing' },
  ];

  const activeContent = $derived.by(() => {
    if (selectedTarget === '__ir__') {
      return irJson ?? '// 正在加载 DocumentIr JSON…';
    }
    const found = openXmlPkg?.files.find((f) => f.path === selectedTarget);
    if (found) return found.xml;
    const first = openXmlPkg?.files[0];
    return first ? first.xml : '';
  });

  const displayedText = $derived.by(() => {
    const q = searchFilter.trim();
    if (!q || !activeContent) return activeContent;
    const lower = q.toLowerCase();
    const lines = activeContent.split('\n');
    const matchedSnippets: string[] = [];
    let i = 0;

    while (i < lines.length) {
      if (lines[i].toLowerCase().includes(lower)) {
        const start = Math.max(0, i - 2);
        let end = Math.min(lines.length - 1, i + 14);
        while (end + 1 < lines.length && end - i < 24 && lines[end + 1].toLowerCase().includes(lower)) {
          end += 1;
        }
        const block = lines
          .slice(start, end + 1)
          .map((ln, offset) => `${String(start + offset + 1).padStart(5, ' ')} │ ${ln}`)
          .join('\n');
        matchedSnippets.push(block);
        i = end + 1;
      } else {
        i += 1;
      }
    }

    if (matchedSnippets.length === 0) {
      return `// 在 ${selectedTarget} 中未匹配到 "${q}"`;
    }
    return `// 共匹配 ${matchedSnippets.length} 处 "${q}" 上下文片段：\n\n` + matchedSnippets.join('\n\n  ⋯⋯⋯\n\n');
  });

  async function copySource() {
    if (!displayedText) return;
    try {
      await navigator.clipboard.writeText(displayedText);
      copied = true;
      setTimeout(() => {
        copied = false;
      }, 1600);
    } catch {
      // Ignore clipboard failures
    }
  }

  function formatKb(bytes: number): string {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
</script>

<div class="xray-root">
  <div class="xray-header">
    <div class="part-tabs">
      {#if openXmlPkg}
        {#each openXmlPkg.files as file (file.path)}
          <button
            type="button"
            class="part-tab mono"
            class:active={selectedTarget === file.path}
            onclick={() => {
              selectedTarget = file.path;
            }}
          >
            <span>{file.path.replace(/^word\//, '')}</span>
          </button>
        {/each}
      {/if}
      <button
        type="button"
        class="part-tab ir-tab"
        class:active={selectedTarget === '__ir__'}
        onclick={() => {
          selectedTarget = '__ir__';
          if (!irJson) onLoadIr();
        }}
      >
        DocumentIr.json
      </button>
    </div>

    <div class="xray-actions tabular">
      {#if openXmlPkg}
        <span class="pkg-size">.docx {formatKb(openXmlPkg.docx_size_bytes)}</span>
      {/if}
      <button type="button" class="action-pill" onclick={onRefresh}>刷新</button>
      <button type="button" class="action-pill" onclick={copySource}>
        {copied ? '已复制 ✓' : '复制'}
      </button>
    </div>
  </div>

  <div class="filter-bar">
    <div class="filter-chips">
      {#each filterPresets as fp (fp.label)}
        <button
          type="button"
          class="filter-chip"
          class:active={searchFilter === fp.query}
          onclick={() => {
            searchFilter = fp.query;
          }}
        >
          {fp.label}
        </button>
      {/each}
    </div>

    <input
      type="search"
      class="filter-input mono"
      placeholder="筛选 XML/JSON 关键字…"
      bind:value={searchFilter}
    />
  </div>

  <div class="code-viewport">
    {#if loading}
      <div class="xray-placeholder">正在构建并解包 OpenXML 部件…</div>
    {:else if error}
      <div class="xray-error mono">{error}</div>
    {:else}
      <pre class="xml-code mono">{displayedText}</pre>
    {/if}
  </div>
</div>

<style>
  .xray-root {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    background: var(--bg-surface);
    overflow: hidden;
  }

  .xray-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 6px 12px;
    background: var(--bg-sidebar);
    border-bottom: 1px solid var(--border-hairline);
    flex-shrink: 0;
  }

  .part-tabs {
    display: flex;
    align-items: center;
    gap: 4px;
    overflow-x: auto;
    min-width: 0;
  }

  .part-tab {
    padding: 4px 9px;
    border-radius: 6px;
    font-size: 11.5px;
    color: var(--text-secondary);
    white-space: nowrap;
    transition: background 0.14s ease, color 0.14s ease;
  }

  .part-tab:hover {
    background: var(--bg-subtle);
    color: var(--text-primary);
  }

  .part-tab.active {
    background: var(--accent-blue-soft);
    color: var(--accent-blue);
    font-weight: 600;
  }

  .ir-tab {
    font-weight: 500;
  }

  .xray-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .pkg-size {
    font-size: 11px;
    color: var(--text-tertiary);
    margin-right: 2px;
  }

  .action-pill {
    height: 24px;
    padding: 0 9px;
    border-radius: 6px;
    background: var(--bg-subtle);
    color: var(--text-primary);
    font-size: 11.5px;
    font-weight: 500;
  }

  .action-pill:hover {
    background: var(--bg-segmented);
  }

  .filter-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 6px 12px;
    background: var(--bg-surface);
    border-bottom: 1px solid var(--border-hairline);
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .filter-chips {
    display: flex;
    align-items: center;
    gap: 5px;
    flex-wrap: wrap;
  }

  .filter-chip {
    padding: 2.5px 8px;
    border-radius: 999px;
    font-size: 11px;
    color: var(--text-secondary);
    background: var(--bg-subtle);
  }

  .filter-chip.active {
    background: var(--accent-blue);
    color: #fff;
    font-weight: 500;
  }

  .filter-input {
    width: 185px;
    height: 25px;
    padding: 0 8px;
    border-radius: 6px;
    border: 1px solid var(--border-hairline);
    background: var(--bg-code);
    font-size: 11.5px;
  }

  .code-viewport {
    flex: 1;
    min-height: 0;
    overflow: auto;
    background: var(--bg-code);
  }

  .xml-code {
    margin: 0;
    padding: 14px 18px 40px 18px;
    font-size: 12px;
    line-height: 1.58;
    color: var(--text-primary);
    white-space: pre;
    tab-size: 2;
  }

  .xray-placeholder,
  .xray-error {
    padding: 28px 20px;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .xray-error {
    color: var(--status-red);
    white-space: pre-wrap;
  }
</style>
