//! Styled container groups (`#block(fill: ..., stroke: ...)[...]`) ->
//! either a breakable 1x1 `BlockNode::Table` (`is_styled_container: true`) or `BlockNode::ShapeBlock`.

use typst::layout::{GridCell, GroupItem, Point};
use typst::model::TableCell;
use typst::pdf::ArtifactElem;
use typst::text::RawElem;
use typst::visualize::{CurveItem, Geometry};

use crate::color::paint_to_hex;
use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::{
    BlockNode, BordersIr, HAlignIr, ShapeIr, ShapeKindIr, SidesPtIr, TableCellIr, TableIr,
    TableRowIr, VAlignIr,
};

impl<'a> ExtractorContext<'a> {
    pub(crate) fn try_extract_styled_container_group(
        &mut self,
        group: &GroupItem,
        _pos: Point,
        children: &[TreeNode],
        list_level: usize,
        inherited_align: HAlignIr,
    ) -> Option<BlockNode> {
        // Check if this group starts with background/border `Shape` items (e.g. `#block(fill: ..., stroke: ...)[...]`)
        // AND is not a raw table/grid cell or CeTZ canvas.
        if Self::is_direct_table_or_code_container(children) || Self::is_cetz_group_nodes(children)
        {
            return None;
        }

        let mut bg_shapes = Vec::new();
        let mut content_nodes = Vec::new();
        for child in children {
            match child {
                TreeNode::Shape { shape, span, pos } => {
                    bg_shapes.push((*pos, shape.clone(), *span));
                }
                TreeNode::Element {
                    elem,
                    children: achildren,
                    pos,
                    ..
                } if elem.is::<ArtifactElem>() => {
                    for ac in achildren {
                        if let TreeNode::Shape {
                            shape,
                            span,
                            pos: spos,
                        } = ac
                        {
                            bg_shapes.push((*pos + *spos, shape.clone(), *span));
                        }
                    }
                }
                other => content_nodes.push(other.clone()),
            }
        }

        if bg_shapes.is_empty() || content_nodes.is_empty() {
            return None;
        }

        let fill = bg_shapes
            .iter()
            .find_map(|(_, s, _)| s.fill.as_ref().and_then(paint_to_hex));
        let stroke = bg_shapes
            .iter()
            .find_map(|(_, s, _)| s.stroke.as_ref().map(Self::fixed_stroke_to_ir));
        if fill.is_none() && stroke.is_none() {
            return None;
        }

        let mut line_starts = Vec::new();
        Self::collect_line_left_edges(&content_nodes, Point::zero(), &mut line_starts);
        let inferred_pad = line_starts
            .iter()
            .map(|(_, lx)| *lx)
            .filter(|&lx| lx >= 2.0)
            .reduce(f64::min)
            .map(|lx| lx.clamp(4.0, 24.0))
            .unwrap_or(6.0);
        let v_pad = inferred_pad.clamp(4.0, 12.0);

        let prev_inset = self.container_inset_pt;
        self.container_inset_pt += 2.0 * inferred_pad + 3.0;
        let body_blocks =
            self.extract_blocks_from_nodes(&content_nodes, list_level, inherited_align);
        self.container_inset_pt = prev_inset;

        if body_blocks.is_empty() {
            return None;
        }

        let size = group.frame.size();
        let width_pt = size.x.to_pt().max(40.0);
        let inset = SidesPtIr {
            top: v_pad,
            bottom: v_pad,
            left: inferred_pad,
            right: inferred_pad,
        };

        // If the styled `#block(stroke: ..., fill: ...)[...]` contains tables, lists, enums,
        // figures, rules, CeTZ canvases, or more than 3 blocks, represent it as a breakable
        // 1-cell bordered `BlockNode::Table` (`is_styled_container: true`) so all inner blocks
        // render natively and flow across columns/pages in Word!
        let needs_table_container = body_blocks.iter().any(|b| {
            !matches!(
                b,
                BlockNode::Paragraph { .. }
                    | BlockNode::Heading { .. }
                    | BlockNode::EquationBlock(_)
                    | BlockNode::CodeBlock { .. }
            )
        }) || body_blocks.len() > 3;

        if needs_table_container {
            self.report.tables_count += 1;
            return Some(BlockNode::Table(TableIr {
                is_semantic_table: false,
                is_styled_container: true,
                col_widths_pt: vec![width_pt],
                row_heights_pt: vec![0.0],
                header_rows: 0,
                rows: vec![TableRowIr {
                    height_pt: None,
                    is_header: false,
                    cells: vec![TableCellIr {
                        x: 0,
                        y: 0,
                        colspan: 1,
                        rowspan: 1,
                        vmerge: None,
                        hmerged_covered: false,
                        fill,
                        borders: BordersIr::splat(stroke),
                        inset,
                        halign: inherited_align,
                        valign: VAlignIr::Top,
                        is_header_cell: false,
                        blocks: body_blocks,
                    }],
                }],
                align: inherited_align,
                left_indent_pt: 0.0,
                summary: None,
            }));
        }

        let corner_radius_pt = bg_shapes.iter().find_map(|(_, s, _)| {
            if let Geometry::Curve(curve) = &s.geometry
                && let Some(CurveItem::Move(p)) = curve.0.first()
            {
                let rx = p.x.to_pt();
                let ry = p.y.to_pt();
                if rx > 0.5 && ry.abs() < 0.5 && rx <= size.x.to_pt() * 0.5 {
                    return Some(rx);
                } else if ry > 0.5 && rx.abs() < 0.5 && ry <= size.y.to_pt() * 0.5 {
                    return Some(ry);
                }
            }
            None
        });

        self.report.shapes_count += 1;
        Some(BlockNode::ShapeBlock(ShapeIr {
            kind: ShapeKindIr::Rect,
            width_pt,
            height_pt: size.y.to_pt().max(20.0),
            fill,
            fill_rule_even_odd: false,
            stroke: stroke.clone(),
            borders: BordersIr::splat(stroke),
            corner_radius_pt,
            inset,
            rotation_deg: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            body: body_blocks,
            align: inherited_align,
        }))
    }

    pub(crate) fn is_direct_table_or_code_container(nodes: &[TreeNode]) -> bool {
        for node in nodes {
            if let TreeNode::Element { elem, .. } = node
                && (elem.is::<TableCell>() || elem.is::<GridCell>() || elem.is::<RawElem>())
            {
                return true;
            }
        }
        false
    }
}
