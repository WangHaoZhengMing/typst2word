<script lang="ts">
  import type { VirtualFile } from '../types';

  interface Props {
    files: VirtualFile[];
    activeFilePath: string;
    mainFilePath: string;
    inputs: Record<string, string>;
    loadedPackages: string[];
    onSelectFile: (path: string) => void;
    onSetMainFile: (path: string) => void;
    onCreateFile: (path: string) => void;
    onDeleteFile: (path: string) => void;
    onUploadProjectFiles: (fileList: FileList) => void;
    onUpdateInputs: (nextInputs: Record<string, string>) => void;
  }

  let {
    files,
    activeFilePath,
    mainFilePath,
    inputs,
    loadedPackages,
    onSelectFile,
    onSetMainFile,
    onCreateFile,
    onDeleteFile,
    onUploadProjectFiles,
    onUpdateInputs,
  }: Props = $props();

  let creatingFile = $state(false);
  let newFileName = $state('');
  let newInputKey = $state('');
  let newInputVal = $state('');
  let uploadInputEl: HTMLInputElement | undefined = $state();

  const inputEntries = $derived(Object.entries(inputs));

  function submitNewFile() {
    const trimmed = newFileName.trim().replace(/^\/+/, '');
    if (!trimmed) {
      creatingFile = false;
      return;
    }
    const finalName = trimmed.includes('.') ? trimmed : `${trimmed}.typ`;
    onCreateFile(finalName);
    newFileName = '';
    creatingFile = false;
  }

  function addSysInput() {
    const k = newInputKey.trim();
    if (!k) return;
    onUpdateInputs({
      ...inputs,
      [k]: newInputVal,
    });
    newInputKey = '';
    newInputVal = '';
  }

  function updateInputVal(key: string, val: string) {
    onUpdateInputs({
      ...inputs,
      [key]: val,
    });
  }

  function removeInputKey(key: string) {
    const next = { ...inputs };
    delete next[key];
    onUpdateInputs(next);
  }

  function formatBytes(len: number): string {
    if (len < 1024) return `${len} B`;
    if (len < 1024 * 1024) return `${(len / 1024).toFixed(1)} KB`;
    return `${(len / (1024 * 1024)).toFixed(1)} MB`;
  }
</script>

<aside class="workspace-sidebar">
  <!-- Section 1: Virtual Project Files -->
  <section class="sidebar-section">
    <div class="section-header">
      <span class="section-title">工程文件</span>
      <div class="section-actions">
        <button
          type="button"
          class="mini-btn"
          title="新建 .typ / .bib 文件"
          onclick={() => {
            creatingFile = true;
            newFileName = 'chapter.typ';
          }}
        >
          + 新建
        </button>
        <button
          type="button"
          class="mini-btn"
          title="导入 .typ / 图片 / .bib / .zip 压缩包"
          onclick={() => uploadInputEl?.click()}
        >
          导入
        </button>
        <input
          bind:this={uploadInputEl}
          type="file"
          multiple
          accept=".typ,.bib,.csv,.json,.toml,.yaml,.yml,.txt,.png,.jpg,.jpeg,.gif,.webp,.svg,.zip"
          hidden
          onchange={(e) => {
            const target = e.currentTarget;
            if (target.files && target.files.length > 0) {
              onUploadProjectFiles(target.files);
              target.value = '';
            }
          }}
        />
      </div>
    </div>

    {#if creatingFile}
      <form
        class="new-file-form"
        onsubmit={(e) => {
          e.preventDefault();
          submitNewFile();
        }}
      >
        <input
          type="text"
          class="text-input mono"
          placeholder="例如 utils.typ 或 refs.bib"
          bind:value={newFileName}
        />
        <div class="form-row-btns">
          <button type="submit" class="tiny-primary">创建</button>
          <button
            type="button"
            class="tiny-ghost"
            onclick={() => {
              creatingFile = false;
            }}
          >
            取消
          </button>
        </div>
      </form>
    {/if}

    <div class="file-tree" role="list">
      {#each files as file (file.path)}
        <div
          class="file-item"
          class:selected={file.path === activeFilePath}
          role="listitem"
        >
          <button
            type="button"
            class="file-select-btn"
            onclick={() => onSelectFile(file.path)}
          >
            <span class="file-ext-dot" class:main={file.path === mainFilePath}></span>
            <span class="file-path mono">{file.path}</span>
            {#if file.path === mainFilePath}
              <span class="entry-badge" title="当前编译入口文件">入口</span>
            {:else if file.text === null}
              <span class="size-hint tabular">{formatBytes(file.bytes.length)}</span>
            {/if}
          </button>

          <div class="file-hover-actions">
            {#if file.path.endsWith('.typ') && file.path !== mainFilePath}
              <button
                type="button"
                class="row-action"
                title="设为编译主入口文件"
                onclick={() => onSetMainFile(file.path)}
              >
                设为主文件
              </button>
            {/if}
            {#if files.length > 1 && file.path !== mainFilePath}
              <button
                type="button"
                class="row-action danger"
                title="删除该文件"
                onclick={() => onDeleteFile(file.path)}
              >
                ×
              </button>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  </section>

  <!-- Section 2: sys.inputs Template Parameters -->
  <section class="sidebar-section">
    <div class="section-header">
      <span class="section-title">模板变量 (sys.inputs)</span>
    </div>

    {#if inputEntries.length > 0}
      <div class="inputs-list">
        {#each inputEntries as [k, v] (k)}
          <div class="input-kv-row">
            <span class="kv-key mono" title={k}>{k}</span>
            <input
              type="text"
              class="kv-val"
              value={v}
              oninput={(e) => updateInputVal(k, e.currentTarget.value)}
            />
            <button
              type="button"
              class="kv-del"
              title="移除变量"
              onclick={() => removeInputKey(k)}
            >
              ×
            </button>
          </div>
        {/each}
      </div>
    {:else}
      <p class="empty-note">可在 Typst 中通过 <code>sys.inputs</code> 读取键值参数。</p>
    {/if}

    <form
      class="add-kv-form"
      onsubmit={(e) => {
        e.preventDefault();
        addSysInput();
      }}
    >
      <input
        type="text"
        class="kv-add-input mono"
        placeholder="key"
        bind:value={newInputKey}
      />
      <input
        type="text"
        class="kv-add-input"
        placeholder="value"
        bind:value={newInputVal}
      />
      <button type="submit" class="kv-add-btn" title="添加参数">+</button>
    </form>
  </section>

  <!-- Section 3: Loaded @preview Packages -->
  <section class="sidebar-section">
    <div class="section-header">
      <span class="section-title">Universe 包缓存</span>
      <span class="pkg-count tabular">{loadedPackages.length}</span>
    </div>
    {#if loadedPackages.length > 0}
      <div class="pkg-chips">
        {#each loadedPackages as pkg (pkg)}
          <span class="pkg-chip mono">{pkg}</span>
        {/each}
      </div>
    {:else}
      <p class="empty-note">
        代码中写 <code>#import "@preview/..."</code> 时将自动按需下载并缓存至本地 IndexedDB。
      </p>
    {/if}
  </section>

  <div class="sidebar-footer">
    <span>支持拖入 <code>.typ</code>、图片、字体或 <code>.zip</code> 工程包</span>
  </div>
</aside>

<style>
  .workspace-sidebar {
    display: flex;
    flex-direction: column;
    width: 236px;
    height: 100%;
    background: var(--bg-sidebar);
    border-right: 1px solid var(--border-hairline);
    flex-shrink: 0;
    overflow-y: auto;
    user-select: none;
  }

  .sidebar-section {
    padding: 12px 10px;
    border-bottom: 1px solid var(--border-hairline);
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 4px 8px 4px;
  }

  .section-title {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--text-tertiary);
  }

  .section-actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .mini-btn {
    font-size: 11px;
    font-weight: 500;
    padding: 2px 6px;
    border-radius: 5px;
    color: var(--accent-blue);
    background: var(--accent-blue-soft);
    transition: opacity 0.14s ease;
  }

  .mini-btn:hover {
    opacity: 0.85;
  }

  .new-file-form {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 6px;
    margin-bottom: 8px;
    border-radius: 8px;
    background: var(--bg-subtle);
  }

  .text-input {
    width: 100%;
    height: 26px;
    padding: 0 8px;
    border-radius: 6px;
    border: 1px solid var(--border-strong);
    background: var(--bg-surface);
    font-size: 11.5px;
  }

  .form-row-btns {
    display: flex;
    gap: 6px;
  }

  .tiny-primary {
    padding: 2px 8px;
    border-radius: 5px;
    background: var(--accent-blue);
    color: #fff;
    font-size: 11px;
    font-weight: 500;
  }

  .tiny-ghost {
    padding: 2px 8px;
    border-radius: 5px;
    color: var(--text-secondary);
    font-size: 11px;
  }

  .file-tree {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .file-item {
    position: relative;
    display: flex;
    align-items: center;
    border-radius: 7px;
    transition: background 0.12s ease;
  }

  .file-item:hover {
    background: var(--bg-subtle);
  }

  .file-item.selected {
    background: var(--accent-blue-soft);
  }

  .file-select-btn {
    display: flex;
    align-items: center;
    gap: 7px;
    flex: 1;
    min-width: 0;
    height: 29px;
    padding: 0 8px;
    text-align: left;
  }

  .file-ext-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--text-tertiary);
    flex-shrink: 0;
  }

  .file-ext-dot.main {
    background: var(--accent-blue);
  }

  .file-path {
    font-size: 12px;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
  }

  .file-item.selected .file-path {
    font-weight: 600;
    color: var(--accent-blue);
  }

  .entry-badge {
    font-size: 10px;
    font-weight: 600;
    padding: 0.5px 5px;
    border-radius: 4px;
    background: var(--accent-blue-soft);
    color: var(--accent-blue);
    flex-shrink: 0;
  }

  .size-hint {
    font-size: 10px;
    color: var(--text-tertiary);
    flex-shrink: 0;
  }

  .file-hover-actions {
    display: none;
    align-items: center;
    gap: 2px;
    padding-right: 6px;
  }

  .file-item:hover .file-hover-actions {
    display: flex;
  }

  .row-action {
    font-size: 10.5px;
    padding: 1px 5px;
    border-radius: 4px;
    color: var(--text-secondary);
    background: var(--bg-surface);
    border: 1px solid var(--border-hairline);
  }

  .row-action.danger:hover {
    color: var(--status-red);
  }

  .inputs-list {
    display: flex;
    flex-direction: column;
    gap: 5px;
    margin-bottom: 8px;
  }

  .input-kv-row {
    display: grid;
    grid-template-columns: 72px 1fr 20px;
    align-items: center;
    gap: 4px;
  }

  .kv-key {
    font-size: 11px;
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    padding-left: 4px;
  }

  .kv-val {
    width: 100%;
    height: 24px;
    padding: 0 6px;
    border-radius: 5px;
    border: 1px solid var(--border-hairline);
    background: var(--bg-surface);
    font-size: 11.5px;
  }

  .kv-del {
    width: 20px;
    height: 20px;
    border-radius: 4px;
    color: var(--text-tertiary);
    font-size: 13px;
  }

  .kv-del:hover {
    color: var(--status-red);
    background: var(--status-red-bg);
  }

  .add-kv-form {
    display: grid;
    grid-template-columns: 68px 1fr 24px;
    gap: 4px;
  }

  .kv-add-input {
    width: 100%;
    height: 25px;
    padding: 0 6px;
    border-radius: 5px;
    border: 1px solid var(--border-hairline);
    background: var(--bg-surface);
    font-size: 11px;
  }

  .kv-add-btn {
    height: 25px;
    border-radius: 5px;
    background: var(--bg-subtle);
    color: var(--text-primary);
    font-weight: 600;
  }

  .kv-add-btn:hover {
    background: var(--accent-blue-soft);
    color: var(--accent-blue);
  }

  .empty-note {
    margin: 0 4px 8px 4px;
    font-size: 11px;
    line-height: 1.48;
    color: var(--text-tertiary);
  }

  .empty-note code {
    font-size: 10.5px;
    padding: 0.5px 3.5px;
    border-radius: 3px;
    background: var(--bg-subtle);
  }

  .pkg-count {
    font-size: 10.5px;
    color: var(--text-tertiary);
  }

  .pkg-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    padding: 0 2px;
  }

  .pkg-chip {
    font-size: 10.5px;
    padding: 2px 7px;
    border-radius: 999px;
    background: var(--status-purple-bg);
    color: var(--status-purple);
  }

  .sidebar-footer {
    margin-top: auto;
    padding: 12px 14px;
    font-size: 11px;
    line-height: 1.45;
    color: var(--text-tertiary);
  }

  .sidebar-footer code {
    font-size: 10.5px;
  }
</style>
