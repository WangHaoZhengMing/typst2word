//! Detection of Typst elements Word cannot represent (`attach`, `html`, `frame`,
//! `skew`, `repeat`, ...) -> `UnsupportedIr` placeholders.

use typst::foundations::{Content, StyleChain};
use typst::introspection::Location;
use typst::layout::{RepeatElem, SkewElem};
use typst::pdf::AttachElem;
use typst::syntax::Span;
use typst_html::{FrameElem, HtmlElem};

use crate::ExtractorContext;
use ir::UnsupportedIr;

impl<'a> ExtractorContext<'a> {
    // =========================================================================
    // Unsupported Element Detection
    // =========================================================================
    pub(crate) fn check_unsupported_element(
        &mut self,
        elem: &Content,
        loc: Location,
    ) -> Option<UnsupportedIr> {
        let source_loc = self.compiled.source_snippet(self.world, elem.span());

        if let Some(attach) = elem.to_packed::<AttachElem>() {
            let path_str = format!("{:?}", attach.path.source);
            let item = UnsupportedIr {
                element_name: "pdf.attach".to_string(),
                reason: format!("PDF 附件嵌入 ({}) 在 Word 文档中不受支持", path_str),
                source_location: source_loc,
            };
            if self.seen_unsupported_locs.insert(loc) {
                self.report.unsupported_elements.push(item.clone());
            }
            return Some(item);
        }

        if let Some(html_elem) = elem.to_packed::<HtmlElem>() {
            let item = UnsupportedIr {
                element_name: format!("html.elem(<{}>)", html_elem.tag),
                reason: "HTML 专属标签元素无法直接转换为 Word 原生语义结构".to_string(),
                source_location: source_loc,
            };
            if self.seen_unsupported_locs.insert(loc) {
                self.report.unsupported_elements.push(item.clone());
            }
            return Some(item);
        }

        if elem.is::<FrameElem>() {
            let item = UnsupportedIr {
                element_name: "html.frame".to_string(),
                reason: "HTML frame 容器为 HTML 导出专属元素".to_string(),
                source_location: source_loc,
            };
            if self.seen_unsupported_locs.insert(loc) {
                self.report.unsupported_elements.push(item.clone());
            }
            return Some(item);
        }

        if let Some(skew) = elem.to_packed::<SkewElem>() {
            let ax = skew.ax.get(StyleChain::default()).to_deg();
            let ay = skew.ay.get(StyleChain::default()).to_deg();
            if ax.abs() > 0.01 || ay.abs() > 0.01 {
                let item = UnsupportedIr {
                    element_name: format!("skew(ax: {:.1}deg, ay: {:.1}deg)", ax, ay),
                    reason: "Word 文本与图形对象不支持 2D 错切(skew)仿射变换".to_string(),
                    source_location: source_loc,
                };
                if self.seen_unsupported_locs.insert(loc) {
                    self.report.unsupported_elements.push(item.clone());
                }
                return Some(item);
            }
        }

        if elem.is::<RepeatElem>() {
            let item = UnsupportedIr {
                element_name: "repeat".to_string(),
                reason: "动态宽度重复填充元素(repeat)在 Word 流式布局中不支持".to_string(),
                source_location: source_loc,
            };
            if self.seen_unsupported_locs.insert(loc) {
                self.report.unsupported_elements.push(item.clone());
            }
            return Some(item);
        }

        None
    }

    pub(crate) fn record_unsupported_paint(
        &mut self,
        context_name: &str,
        span: Span,
    ) -> UnsupportedIr {
        let item = UnsupportedIr {
            element_name: format!("tiling/pattern ({})", context_name),
            reason: "Word 原生图形不支持 Typst 自定义平铺图案(tiling/pattern)填充".to_string(),
            source_location: self.compiled.source_snippet(self.world, span),
        };
        self.report.unsupported_elements.push(item.clone());
        item
    }
}
