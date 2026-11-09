//! Cell node collection, border extraction, and cell property helpers for tables/grids.

use std::collections::HashMap;

use typst::foundations::{Content, Smart, StyleChain};
use typst::layout::grid::resolve::Cell;
use typst::layout::{GridCell, GridElem};
use typst::model::{EnumElem, ListElem, TableCell, TableElem};
use typst::pdf::TableCellKind;

use super::CellNodeMap;
use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::{BordersIr, HAlignIr, SidesPtIr, VAlignIr};

impl<'a> ExtractorContext<'a> {
    pub(crate) fn collect_table_cell_nodes(nodes: &[TreeNode], out: &mut CellNodeMap) {
        for node in nodes {
            match node {
                TreeNode::Element { elem, children, .. } => {
                    if let Some(tc) = elem.to_packed::<TableCell>() {
                        if tc.is_repeated.get(StyleChain::default()) {
                            continue;
                        }
                        if let (Smart::Custom(x), Smart::Custom(y)) = (
                            tc.x.get(StyleChain::default()),
                            tc.y.get(StyleChain::default()),
                        ) {
                            let size = Self::find_group_size(children);
                            out.entry((x, y))
                                .and_modify(|(_, existing_children, _)| {
                                    existing_children.extend(children.iter().cloned());
                                })
                                .or_insert_with(|| (elem.clone(), children.clone(), size));
                        }
                        continue;
                    }
                    if let Some(gc) = elem.to_packed::<GridCell>() {
                        if gc.is_repeated.get(StyleChain::default()) {
                            continue;
                        }
                        if let (Smart::Custom(x), Smart::Custom(y)) = (
                            gc.x.get(StyleChain::default()),
                            gc.y.get(StyleChain::default()),
                        ) {
                            let size = Self::find_group_size(children);
                            out.entry((x, y))
                                .and_modify(|(_, existing_children, _)| {
                                    existing_children.extend(children.iter().cloned());
                                })
                                .or_insert_with(|| (elem.clone(), children.clone(), size));
                        }
                        continue;
                    }
                    if elem.is::<TableElem>()
                        || elem.is::<GridElem>()
                        || elem.is::<ListElem>()
                        || elem.is::<EnumElem>()
                    {
                        continue;
                    }
                    Self::collect_table_cell_nodes(children, out);
                }
                TreeNode::Group {
                    group, children, ..
                } => {
                    if children.len() == 1
                        && let TreeNode::Element { elem, .. } = &children[0]
                        && (elem.is::<TableCell>() || elem.is::<GridCell>())
                    {
                        let mut temp = HashMap::new();
                        Self::collect_table_cell_nodes(children, &mut temp);
                        for ((x, y), (ce, cc, sz)) in temp {
                            let final_sz = if sz.x.to_pt() > 0.0 {
                                sz
                            } else {
                                group.frame.size()
                            };
                            out.entry((x, y))
                                .and_modify(|(_, existing_children, _)| {
                                    existing_children.extend(cc.iter().cloned());
                                })
                                .or_insert((ce, cc, final_sz));
                        }
                        continue;
                    }
                    Self::collect_table_cell_nodes(children, out);
                }
                _ => {}
            }
        }
    }

    pub(crate) fn nodes_contain_list(nodes: &[TreeNode]) -> bool {
        nodes.iter().any(|n| match n {
            TreeNode::Element { elem, children, .. } => {
                elem.is::<ListElem>() || elem.is::<EnumElem>() || Self::nodes_contain_list(children)
            }
            TreeNode::Group { children, .. } => Self::nodes_contain_list(children),
            _ => false,
        })
    }

    pub(crate) fn get_cell_colspan(elem: &Content) -> usize {
        elem.to_packed::<TableCell>()
            .map(|tc| tc.colspan.get(StyleChain::default()).get())
            .or_else(|| {
                elem.to_packed::<GridCell>()
                    .map(|gc| gc.colspan.get(StyleChain::default()).get())
            })
            .unwrap_or(1)
    }

    pub(crate) fn get_cell_rowspan(elem: &Content) -> usize {
        elem.to_packed::<TableCell>()
            .map(|tc| tc.rowspan.get(StyleChain::default()).get())
            .or_else(|| {
                elem.to_packed::<GridCell>()
                    .map(|gc| gc.rowspan.get(StyleChain::default()).get())
            })
            .unwrap_or(1)
    }

    pub(crate) fn extract_cell_borders(cell: &Cell, _is_semantic_table: bool) -> BordersIr {
        BordersIr {
            top: cell.stroke.top.as_deref().map(Self::abs_stroke_to_ir),
            bottom: cell.stroke.bottom.as_deref().map(Self::abs_stroke_to_ir),
            left: cell.stroke.left.as_deref().map(Self::abs_stroke_to_ir),
            right: cell.stroke.right.as_deref().map(Self::abs_stroke_to_ir),
        }
    }

    pub(crate) fn extract_cell_props(
        elem: &Content,
        default_is_header: bool,
        is_semantic_table: bool,
    ) -> (SidesPtIr, HAlignIr, VAlignIr, bool) {
        let grid_def = SidesPtIr::zero();
        if let Some(tc) = elem.to_packed::<TableCell>() {
            let (ha, va) = Self::alignment_to_ir(tc.align.get(StyleChain::default()));
            let inset = Self::sides_rel_length_to_pt(&tc.inset.get(StyleChain::default()));
            let is_header = matches!(
                tc.kind.get(StyleChain::default()),
                Smart::Custom(TableCellKind::Header(..))
            ) || default_is_header;
            (inset, ha, va, is_header)
        } else if let Some(gc) = elem.to_packed::<GridCell>() {
            let (ha, va) = Self::alignment_to_ir(gc.align.get(StyleChain::default()));
            let inset = Self::sides_rel_length_to_pt_with_default(
                &gc.inset.get(StyleChain::default()),
                grid_def,
            );
            (inset, ha, va, default_is_header)
        } else {
            let inset = if is_semantic_table {
                SidesPtIr::default()
            } else {
                grid_def
            };
            (inset, HAlignIr::Left, VAlignIr::Top, default_is_header)
        }
    }
}
