<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Compartment, EditorState } from '@codemirror/state';
  import {
    EditorView,
    highlightActiveLine,
    highlightActiveLineGutter,
    keymap,
    lineNumbers,
    drawSelection,
  } from '@codemirror/view';
  import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
  import {
    HighlightStyle,
    StreamLanguage,
    bracketMatching,
    syntaxHighlighting,
  } from '@codemirror/language';
  import { searchKeymap } from '@codemirror/search';
  import { tags } from '@lezer/highlight';

  interface Props {
    value: string;
    filePath: string;
    readOnly?: boolean;
    theme: 'light' | 'dark';
    compileError: string | null;
    onChange: (nextValue: string) => void;
    onExportShortcut: () => void;
  }

  let {
    value,
    filePath,
    readOnly = false,
    theme,
    compileError,
    onChange,
    onExportShortcut,
  }: Props = $props();

  let hostEl: HTMLDivElement | undefined = $state();
  let view: EditorView | null = null;
  let themeCompartment = new Compartment();
  let readOnlyCompartment = new Compartment();
  let suppressExternalSync = false;
  let cursorPos = $state({ line: 1, col: 1 });

  interface TypstLexerState {
    inBlockComment: boolean;
    inMath: boolean;
    inRawBlock: boolean;
  }

  const typstLanguage = StreamLanguage.define<TypstLexerState>({
    startState: () => ({
      inBlockComment: false,
      inMath: false,
      inRawBlock: false,
    }),
    token(stream, state) {
      if (state.inBlockComment) {
        if (stream.skipTo('*/')) {
          stream.next();
          stream.next();
          state.inBlockComment = false;
        } else {
          stream.skipToEnd();
        }
        return 'comment';
      }

      if (state.inRawBlock) {
        if (stream.match('```')) {
          state.inRawBlock = false;
          return 'monospace';
        }
        stream.next();
        return 'monospace';
      }

      if (stream.match('//')) {
        stream.skipToEnd();
        return 'comment';
      }

      if (stream.match('/*')) {
        state.inBlockComment = true;
        return 'comment';
      }

      if (stream.match('```')) {
        state.inRawBlock = true;
        stream.eatWhile(/[a-zA-Z0-9_-]/);
        return 'monospace';
      }

      if (stream.peek() === '$') {
        stream.next();
        state.inMath = !state.inMath;
        return 'keyword';
      }

      if (state.inMath) {
        if (stream.match(/^\\[a-zA-Z]+/) || stream.match(/^[a-zA-Z_][a-zA-Z0-9_.]*/)) {
          return 'atom';
        }
        if (stream.match(/^[0-9]+(\.[0-9]+)?/)) {
          return 'number';
        }
        stream.next();
        return 'atom';
      }

      if (stream.sol()) {
        if (stream.match(/^\s*=+\s.*/)) {
          return 'heading';
        }
        if (stream.match(/^\s*[-+/]\s/)) {
          return 'punctuation';
        }
      }

      if (stream.peek() === '"') {
        stream.next();
        while (!stream.eol()) {
          const ch = stream.next();
          if (ch === '\\') {
            stream.next();
          } else if (ch === '"') {
            break;
          }
        }
        return 'string';
      }

      if (stream.peek() === '`') {
        stream.next();
        while (!stream.eol()) {
          if (stream.next() === '`') break;
        }
        return 'monospace';
      }

      if (stream.match(/^#(set|show|let|import|include|if|else|for|while|in|return|context)\b/)) {
        return 'keyword';
      }

      if (stream.match(/^#[a-zA-Z_][a-zA-Z0-9_-]*/)) {
        return 'function';
      }

      if (stream.match(/^@[a-zA-Z0-9_:-]+/) || stream.match(/^<[a-zA-Z0-9_:-]+>/)) {
        return 'labelName';
      }

      if (stream.match(/^\b[0-9]+(\.[0-9]+)?(pt|mm|cm|in|em|fr|%|deg|rad)?\b/)) {
        return 'number';
      }

      if (stream.match(/^\*[^*\n]+\*/)) {
        return 'strong';
      }

      if (stream.match(/^_[^_\n]+_/)) {
        return 'emphasis';
      }

      stream.next();
      return null;
    },
  });

  function buildThemeExtension(mode: 'light' | 'dark') {
    const isDark = mode === 'dark';
    const editorTheme = EditorView.theme(
      {
        '&': {
          height: '100%',
          backgroundColor: isDark ? '#1c1c1e' : '#ffffff',
          color: isDark ? '#f5f5f7' : '#1d1d1f',
          fontSize: '13px',
        },
        '.cm-scroller': {
          fontFamily:
            "'SF Mono', ui-monospace, 'JetBrains Mono', Menlo, Monaco, 'PingFang SC', monospace",
          lineHeight: '1.62',
          padding: '10px 0 36px 0',
        },
        '.cm-content': {
          padding: '0 16px 0 4px',
          caretColor: isDark ? '#0a84ff' : '#0071e3',
        },
        '.cm-gutters': {
          backgroundColor: isDark ? '#1c1c1e' : '#ffffff',
          color: isDark ? '#58585d' : '#aeaeb2',
          borderRight: 'none',
          paddingLeft: '8px',
          paddingRight: '6px',
          userSelect: 'none',
        },
        '.cm-activeLineGutter': {
          backgroundColor: 'transparent',
          color: isDark ? '#a1a1a6' : '#515154',
          fontWeight: '600',
        },
        '.cm-activeLine': {
          backgroundColor: isDark ? 'rgba(255, 255, 255, 0.035)' : 'rgba(0, 113, 227, 0.035)',
        },
        '.cm-selectionBackground, ::selection': {
          backgroundColor: isDark
            ? 'rgba(10, 132, 255, 0.30) !important'
            : 'rgba(0, 113, 227, 0.17) !important',
        },
        '&.cm-focused': {
          outline: 'none',
        },
        '.cm-matchingBracket': {
          backgroundColor: isDark ? 'rgba(10, 132, 255, 0.28)' : 'rgba(0, 113, 227, 0.15)',
          outline: 'none',
          borderRadius: '2px',
        },
      },
      { dark: isDark },
    );

    const highlight = HighlightStyle.define([
      {
        tag: tags.heading,
        color: isDark ? '#5ac8fa' : '#0055d4',
        fontWeight: '700',
      },
      {
        tag: tags.keyword,
        color: isDark ? '#ff7ab2' : '#ad3da4',
        fontWeight: '600',
      },
      {
        tag: tags.function(tags.variableName),
        color: isDark ? '#b281eb' : '#4b21b0',
      },
      {
        tag: tags.atom,
        color: isDark ? '#6bd9a8' : '#146c43',
      },
      {
        tag: tags.string,
        color: isDark ? '#fc6a5d' : '#c41a16',
      },
      {
        tag: tags.monospace,
        color: isDark ? '#ffa14f' : '#824300',
      },
      {
        tag: tags.labelName,
        color: isDark ? '#4eb0ff' : '#0066cc',
        fontWeight: '500',
      },
      {
        tag: tags.number,
        color: isDark ? '#d0bf69' : '#1c00cf',
      },
      {
        tag: tags.comment,
        color: isDark ? '#7f8c98' : '#707f8c',
        fontStyle: 'italic',
      },
      {
        tag: tags.strong,
        fontWeight: '700',
      },
      {
        tag: tags.emphasis,
        fontStyle: 'italic',
      },
    ]);

    return [editorTheme, syntaxHighlighting(highlight)];
  }

  function insertSnippet(snippet: string) {
    if (!view || readOnly) return;
    const sel = view.state.selection.main;
    view.dispatch({
      changes: { from: sel.from, to: sel.to, insert: snippet },
      selection: { anchor: sel.from + snippet.length },
    });
    view.focus();
  }

  onMount(() => {
    if (!hostEl) return;

    const state = EditorState.create({
      doc: value,
      extensions: [
        lineNumbers(),
        highlightActiveLineGutter(),
        highlightActiveLine(),
        history(),
        drawSelection(),
        bracketMatching(),
        EditorView.lineWrapping,
        typstLanguage,
        themeCompartment.of(buildThemeExtension(theme)),
        readOnlyCompartment.of(EditorState.readOnly.of(readOnly)),
        keymap.of([
          {
            key: 'Mod-s',
            run: () => {
              onExportShortcut();
              return true;
            },
          },
          {
            key: 'Mod-e',
            run: () => {
              onExportShortcut();
              return true;
            },
          },
          indentWithTab,
          ...defaultKeymap,
          ...historyKeymap,
          ...searchKeymap,
        ]),
        EditorView.updateListener.of((update) => {
          if (update.selectionSet || update.docChanged) {
            const head = update.state.selection.main.head;
            const lineObj = update.state.doc.lineAt(head);
            cursorPos = {
              line: lineObj.number,
              col: head - lineObj.from + 1,
            };
          }
          if (update.docChanged && !suppressExternalSync) {
            onChange(update.state.doc.toString());
          }
        }),
      ],
    });

    view = new EditorView({
      state,
      parent: hostEl,
    });
  });

  $effect(() => {
    const nextDoc = value;
    if (view && view.state.doc.toString() !== nextDoc) {
      suppressExternalSync = true;
      view.dispatch({
        changes: {
          from: 0,
          to: view.state.doc.length,
          insert: nextDoc,
        },
      });
      suppressExternalSync = false;
    }
  });

  $effect(() => {
    const currentTheme = theme;
    if (view) {
      view.dispatch({
        effects: themeCompartment.reconfigure(buildThemeExtension(currentTheme)),
      });
    }
  });

  $effect(() => {
    const isRo = readOnly;
    if (view) {
      view.dispatch({
        effects: readOnlyCompartment.reconfigure(EditorState.readOnly.of(isRo)),
      });
    }
  });

  onDestroy(() => {
    view?.destroy();
    view = null;
  });
</script>

<div class="editor-shell">
  <div class="editor-header">
    <div class="file-crumb">
      <svg class="crumb-icon" viewBox="0 0 16 16" fill="none" aria-hidden="true">
        <path
          d="M4 2.25h5.2L12.5 5.5v8.25c0 .55-.45 1-1 1h-7.5c-.55 0-1-.45-1-1V3.25c0-.55.45-1 1-1Z"
          stroke="currentColor"
          stroke-width="1.2"
        />
        <path d="M9 2.5v3.25h3.25" stroke="currentColor" stroke-width="1.2" />
      </svg>
      <span class="file-name mono">{filePath}</span>
      {#if readOnly}
        <span class="ro-pill">二进制只读</span>
      {/if}
    </div>

    {#if !readOnly && filePath.endsWith('.typ')}
      <div class="quick-snippets" aria-label="快捷插入">
        <button
          type="button"
          class="snippet-chip"
          title="插入编号 OMML 公式"
          onclick={() =>
            insertSnippet('\n$ integral_0^infinity e^(-x^2) dif x = sqrt(pi) / 2 $\n')}
        >
          + 公式
        </button>
        <button
          type="button"
          class="snippet-chip"
          title="插入科研三线表"
          onclick={() =>
            insertSnippet(
              '\n#figure(\n  table(\n    columns: (1fr, 1fr, 1fr),\n    stroke: (x, y) => (\n      top: if y == 0 { 1.2pt } else if y == 1 { 0.6pt } else { none },\n      bottom: 1.2pt,\n    ),\n    table.header([*指标*], [*实验组*], [*对照组*]),\n    [准确率], [98.4%], [94.1%],\n  ),\n  caption: [实验结果对比],\n)\n',
            )}
        >
          + 三线表
        </button>
        <button
          type="button"
          class="snippet-chip"
          title="插入原生 DrawingML 图形"
          onclick={() =>
            insertSnippet(
              '\n#rect(width: 180pt, height: 36pt, fill: rgb("#E8F2FF"), stroke: 1.2pt + rgb("#0071E3"), radius: 6pt)[\n  #align(center + horizon)[*原生形状* `<wps:wsp>`]\n]\n',
            )}
        >
          + 矢量框
        </button>
      </div>
    {/if}
  </div>

  <div class="editor-body" bind:this={hostEl}></div>

  {#if compileError}
    <div class="diagnostic-sheet" role="alert">
      <div class="diag-title">
        <span class="diag-dot"></span>
        <span>Typst 编译诊断</span>
      </div>
      <pre class="diag-text mono">{compileError}</pre>
    </div>
  {/if}

  <div class="editor-statusbar tabular">
    <span>UTF-8 · Typst 0.15</span>
    <span>行 {cursorPos.line}，列 {cursorPos.col}</span>
  </div>
</div>

<style>
  .editor-shell {
    position: relative;
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    background: var(--bg-surface);
    overflow: hidden;
  }

  .editor-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    height: 36px;
    padding: 0 12px;
    border-bottom: 1px solid var(--border-hairline);
    background: var(--bg-sidebar);
    flex-shrink: 0;
  }

  .file-crumb {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    color: var(--text-secondary);
  }

  .crumb-icon {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    color: var(--accent-blue);
  }

  .file-name {
    font-size: 12px;
    font-weight: 500;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ro-pill {
    font-size: 10.5px;
    padding: 1px 6px;
    border-radius: 999px;
    background: var(--bg-subtle);
    color: var(--text-tertiary);
  }

  .quick-snippets {
    display: flex;
    align-items: center;
    gap: 5px;
    flex-shrink: 0;
  }

  .snippet-chip {
    padding: 2px 8px;
    border-radius: 6px;
    font-size: 11.5px;
    font-weight: 500;
    color: var(--text-secondary);
    background: var(--bg-subtle);
    transition: background 0.15s ease, color 0.15s ease;
  }

  .snippet-chip:hover {
    background: var(--accent-blue-soft);
    color: var(--accent-blue);
  }

  .editor-body {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .editor-body :global(.cm-editor) {
    height: 100%;
  }

  .diagnostic-sheet {
    margin: 0 12px 10px 12px;
    padding: 10px 12px;
    border-radius: 10px;
    background: var(--status-red-bg);
    border: 1px solid rgba(255, 59, 48, 0.26);
    max-height: 150px;
    overflow: auto;
    backdrop-filter: blur(14px);
    flex-shrink: 0;
  }

  .diag-title {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--status-red);
    margin-bottom: 4px;
  }

  .diag-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--status-red);
  }

  .diag-text {
    margin: 0;
    font-size: 11.5px;
    line-height: 1.48;
    white-space: pre-wrap;
    word-break: break-word;
    color: var(--text-primary);
  }

  .editor-statusbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 25px;
    padding: 0 12px;
    border-top: 1px solid var(--border-hairline);
    background: var(--bg-sidebar);
    font-size: 11px;
    color: var(--text-tertiary);
    flex-shrink: 0;
  }
</style>
