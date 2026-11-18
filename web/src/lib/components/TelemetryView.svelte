<script lang="ts">
  import type { ConversionReport, FontFamilyDto, StageMode } from '../types';

  interface Props {
    report: ConversionReport | null;
    compilerWarnings: string[];
    fonts: FontFamilyDto[];
    supportsLocalFontAccess: boolean;
    onSyncLocalFonts: () => void;
    onOpenXRay: (filterTag?: string) => void;
    onExportDocx: () => void;
    onBackPreview: (mode: StageMode) => void;
  }

  let {
    report,
    compilerWarnings,
    fonts,
    supportsLocalFontAccess,
    onSyncLocalFonts,
    onOpenXRay,
    onExportDocx,
    onBackPreview,
  }: Props = $props();

  const metrics = $derived(
    report
      ? [
          {
            label: 'Office Math 公式',
            xmlTag: '<m:oMath',
            sub: '原生可编辑 OMML 公式树',
            count: report.equations_count,
            highlight: true,
          },
          {
            label: '结构化表格',
            xmlTag: '<w:tbl',
            sub: '跨行列合并与独立四边边框',
            count: report.tables_count,
            highlight: true,
          },
          {
            label: 'DrawingML 几何图形',
            xmlTag: '<wps:wsp',
            sub: '原生自定义路径与文本框',
            count: report.shapes_count,
            highlight: true,
          },
          {
            label: 'CeTZ 矢量画布',
            xmlTag: '<wpg:wgp',
            sub: '组合矢量对象（零位图降级）',
            count: report.cetz_canvases_count,
            highlight: true,
          },
          {
            label: '大纲章节标题',
            xmlTag: 'Heading',
            sub: '原生 Heading 样式与书签锚点',
            count: report.headings_count,
            highlight: false,
          },
          {
            label: '正文校准段落',
            xmlTag: '<w:spacing',
            sub: 'OS/2 行盒与 Knuth-Plass 字距补偿',
            count: report.paragraphs_count,
            highlight: false,
          },
          {
            label: '多级编号与列表',
            xmlTag: '<w:numPr',
            sub: '无序 / 有序 / 术语定义列表',
            count: report.lists_count,
            highlight: false,
          },
          {
            label: '语法高亮代码块',
            xmlTag: '<w:shd',
            sub: '保留 Token 着色与等宽紧凑行距',
            count: report.code_blocks_count,
            highlight: false,
          },
          {
            label: '题注图表 / 引用块',
            xmlTag: 'SEQ',
            sub: `图表 ${report.figures_count} · 引用 ${report.quotes_count} · 图片 ${report.images_count}`,
            count: report.figures_count + report.quotes_count + report.images_count,
            highlight: false,
          },
          {
            label: '脚注与超链接引用',
            xmlTag: 'hyperlink',
            sub: `原生脚注 ${report.footnotes_count} · 链接 ${report.links_count}`,
            count: report.footnotes_count + report.links_count,
            highlight: false,
          },
        ]
      : [],
  );

  const fontWarnings = $derived(report?.font_fallback_warnings ?? []);
  const unsupported = $derived(report?.unsupported_elements ?? []);
  const systemFontsCount = $derived(fonts.filter((f) => f.is_system_installed).length);
</script>

<div class="telemetry-scroll">
  <div class="telemetry-container">
    <!-- Pipeline Banner -->
    <section class="hero-card">
      <div class="hero-top">
        <div>
          <h2 class="hero-title">Word 语义构造与视觉补偿体检</h2>
          <p class="hero-subtitle">
            不同于将版面拍平为文本框或图片的 PDF 转换器，<code>typst2word</code>
            同时提取语义树与物理坐标，直接降维为原生可编辑的 OpenXML 对象。
          </p>
        </div>
        <div class="hero-actions">
          <button
            type="button"
            class="secondary-btn"
            onclick={() => onBackPreview('preview')}
          >
            返回画布
          </button>
          <button type="button" class="primary-btn" onclick={onExportDocx}>
            导出 .docx
          </button>
        </div>
      </div>

      <div class="pipeline-flow" aria-label="5阶段转换流水线">
        <div class="stage-pill">
          <span class="stage-num">1</span>
          <div>
            <div class="stage-name">Compiler</div>
            <div class="stage-desc">AST + Frame 双源钩子</div>
          </div>
        </div>
        <span class="stage-arrow">→</span>
        <div class="stage-pill">
          <span class="stage-num">2</span>
          <div>
            <div class="stage-name">Extract & IR</div>
            <div class="stage-desc">语义 + OpenType 度量</div>
          </div>
        </div>
        <span class="stage-arrow">→</span>
        <div class="stage-pill">
          <span class="stage-num">3</span>
          <div>
            <div class="stage-name">Word Construct</div>
            <div class="stage-desc">1.3× 行盒与段距求解</div>
          </div>
        </div>
        <span class="stage-arrow">→</span>
        <div class="stage-pill">
          <span class="stage-num">4</span>
          <div>
            <div class="stage-name">Docx Writer</div>
            <div class="stage-desc">OMML + DrawingML 封装</div>
          </div>
        </div>
      </div>
    </section>

    {#if report}
      <!-- Construct Bento Grid -->
      <section class="section-block">
        <div class="block-heading">
          <h3>原生 OpenXML 目标对象统计</h3>
          <span class="hint-text">点击任意卡片可直接定位对应的 OpenXML 源码标签</span>
        </div>

        <div class="bento-grid tabular">
          {#each metrics as item (item.label)}
            <button
              type="button"
              class="bento-card"
              class:featured={item.highlight && item.count > 0}
              onclick={() => onOpenXRay(item.xmlTag)}
            >
              <div class="card-top">
                <span class="card-count">{item.count}</span>
                <code class="card-tag">{item.xmlTag}</code>
              </div>
              <div class="card-label">{item.label}</div>
              <div class="card-sub">{item.sub}</div>
            </button>
          {/each}
        </div>
      </section>

      <!-- Font Calibration & Warnings -->
      <section class="section-block">
        <div class="block-heading">
          <h3>字体槽位路由与行高校准诊断</h3>
          {#if supportsLocalFontAccess}
            <button type="button" class="link-action" onclick={onSyncLocalFonts}>
              {systemFontsCount > 0
                ? `已同步 ${systemFontsCount} 款本机字体 · 重新同步`
                : '授权同步本机字体以对齐桌面 Word 度量 →'}
            </button>
          {/if}
        </div>

        <div class="diagnostics-card">
          {#if fontWarnings.length === 0 && compilerWarnings.length === 0 && unsupported.length === 0}
            <div class="clean-pass">
              <span class="pass-badge">✓ 零回退告警</span>
              <span>
                所有字体族、数学公式与矢量图形均已完整匹配并精确完成行盒校准（共 {report.pages_count}
                页）。
              </span>
            </div>
          {:else}
            {#if fontWarnings.length > 0}
              <div class="warn-group">
                <div class="warn-group-title">
                  字体回退提示（生成的 .docx 仍原样保留您指定的字体名称）
                </div>
                {#each fontWarnings as w, idx (idx)}
                  <div class="warn-item mono">{w}</div>
                {/each}
              </div>
            {/if}

            {#if compilerWarnings.length > 0}
              <div class="warn-group">
                <div class="warn-group-title">Typst 编译器提示</div>
                {#each compilerWarnings as w, idx (idx)}
                  <div class="warn-item mono">{w}</div>
                {/each}
              </div>
            {/if}

            {#if unsupported.length > 0}
              <div class="warn-group">
                <div class="warn-group-title">
                  安全兜底占位元素（已在 Word 对应位置生成显式提示块）
                </div>
                {#each unsupported as u, idx (idx)}
                  <div class="warn-item">
                    <strong>{u.element_name}</strong>
                    {#if u.reason}
                      <span> — {u.reason}</span>
                    {/if}
                    {#if u.source_location}
                      <code class="loc-chip">{u.source_location}</code>
                    {/if}
                  </div>
                {/each}
              </div>
            {/if}
          {/if}
        </div>
      </section>
    {:else}
      <div class="empty-state">正在等待首次编译完成以生成体检报告…</div>
    {/if}
  </div>
</div>

<style>
  .telemetry-scroll {
    width: 100%;
    height: 100%;
    overflow-y: auto;
    background: var(--bg-app);
    padding: 22px 24px 44px 24px;
  }

  .telemetry-container {
    max-width: 880px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    gap: 22px;
  }

  .hero-card {
    background: var(--bg-surface);
    border: 1px solid var(--border-hairline);
    border-radius: 16px;
    padding: 20px 22px;
    box-shadow: var(--shadow-card);
  }

  .hero-top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 16px;
  }

  .hero-title {
    margin: 0 0 6px 0;
    font-size: 17px;
    font-weight: 650;
    letter-spacing: -0.02em;
    color: var(--text-primary);
  }

  .hero-subtitle {
    margin: 0;
    font-size: 12.5px;
    line-height: 1.55;
    color: var(--text-secondary);
    max-width: 580px;
  }

  .hero-subtitle code {
    font-size: 11.5px;
    padding: 1px 5px;
    border-radius: 4px;
    background: var(--bg-subtle);
  }

  .hero-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .primary-btn {
    height: 30px;
    padding: 0 13px;
    border-radius: 999px;
    background: var(--accent-blue);
    color: #fff;
    font-size: 12px;
    font-weight: 600;
  }

  .primary-btn:hover {
    background: var(--accent-blue-hover);
  }

  .secondary-btn {
    height: 30px;
    padding: 0 12px;
    border-radius: 999px;
    background: var(--bg-subtle);
    color: var(--text-primary);
    font-size: 12px;
    font-weight: 500;
  }

  .secondary-btn:hover {
    background: var(--bg-segmented);
  }

  .pipeline-flow {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    padding-top: 14px;
    border-top: 1px solid var(--border-hairline);
  }

  .stage-pill {
    display: flex;
    align-items: center;
    gap: 9px;
    flex: 1;
    min-width: 155px;
    padding: 8px 11px;
    border-radius: 10px;
    background: var(--bg-subtle);
  }

  .stage-num {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: var(--accent-blue);
    color: #fff;
    font-size: 11px;
    font-weight: 700;
    flex-shrink: 0;
  }

  .stage-name {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .stage-desc {
    font-size: 11px;
    color: var(--text-secondary);
  }

  .stage-arrow {
    color: var(--text-tertiary);
    font-size: 13px;
  }

  .section-block {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .block-heading {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
  }

  .block-heading h3 {
    margin: 0;
    font-size: 13.5px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .hint-text {
    font-size: 11.5px;
    color: var(--text-tertiary);
  }

  .link-action {
    font-size: 12px;
    font-weight: 500;
    color: var(--accent-blue);
  }

  .link-action:hover {
    text-decoration: underline;
  }

  .bento-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(195px, 1fr));
    gap: 10px;
  }

  .bento-card {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    text-align: left;
    padding: 13px 14px;
    border-radius: 13px;
    background: var(--bg-surface);
    border: 1px solid var(--border-hairline);
    box-shadow: var(--shadow-card);
    transition: transform 0.14s ease, border-color 0.14s ease;
  }

  .bento-card:hover {
    transform: translateY(-1px);
    border-color: var(--accent-blue);
  }

  .bento-card.featured {
    background: linear-gradient(180deg, var(--accent-blue-soft), var(--bg-surface) 65%);
  }

  .card-top {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin-bottom: 6px;
  }

  .card-count {
    font-size: 24px;
    font-weight: 700;
    letter-spacing: -0.03em;
    color: var(--text-primary);
  }

  .bento-card.featured .card-count {
    color: var(--accent-blue);
  }

  .card-tag {
    font-size: 10.5px;
    padding: 1.5px 6px;
    border-radius: 5px;
    background: var(--bg-subtle);
    color: var(--text-secondary);
  }

  .card-label {
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text-primary);
    margin-bottom: 2px;
  }

  .card-sub {
    font-size: 11px;
    color: var(--text-secondary);
    line-height: 1.4;
  }

  .diagnostics-card {
    background: var(--bg-surface);
    border: 1px solid var(--border-hairline);
    border-radius: 14px;
    padding: 14px 16px;
    box-shadow: var(--shadow-card);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .clean-pass {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12.5px;
    color: var(--text-secondary);
  }

  .pass-badge {
    font-size: 11.5px;
    font-weight: 600;
    padding: 2.5px 9px;
    border-radius: 999px;
    background: var(--status-green-bg);
    color: var(--status-green);
    flex-shrink: 0;
  }

  .warn-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .warn-group-title {
    font-size: 11.5px;
    font-weight: 600;
    color: var(--status-amber);
  }

  .warn-item {
    padding: 7px 10px;
    border-radius: 8px;
    background: var(--status-amber-bg);
    font-size: 11.5px;
    color: var(--text-primary);
    line-height: 1.45;
  }

  .loc-chip {
    margin-left: 6px;
    font-size: 10.5px;
    padding: 1px 5px;
    border-radius: 4px;
    background: var(--bg-surface);
  }

  .empty-state {
    text-align: center;
    padding: 40px 0;
    color: var(--text-tertiary);
  }
</style>
