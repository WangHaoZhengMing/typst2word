//! Styled container groups (`#block(fill: ..., stroke: ...)[...]`) ->
//! either a breakable 1x1 `BlockNode::Table` (`is_styled_container: true`) or `BlockNode::ShapeBlock`.

use typst::layout::{GridCell, GroupItem, Point, Size};
use typst::model::TableCell;
use typst::pdf::ArtifactElem;
use typst::text::RawElem;
use typst::visualize::{CurveItem, Geometry};

use crate::color::paint_to_hex;
use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use ir::{
    BlockNode, BordersIr, DashPatternIr, HAlignIr, ShapeIr, ShapeKindIr, SidesPtIr, StrokeIr,
    TableCellIr, TableIr, TableKindIr, TableRowIr, VAlignIr,
};

impl<'a> ExtractorContext<'a> {
    pub(crate) fn try_extract_styled_container_group(
        &mut self,
        group: &GroupItem,
        pos: Point,
        children: &[TreeNode],
        list_level: usize,
        inherited_align: HAlignIr,
    ) -> Option<BlockNode> {
        self.try_extract_styled_container_nodes(
            Some(group.frame.size()),
            pos,
            children,
            list_level,
            inherited_align,
        )
    }

    pub(crate) fn try_extract_styled_container_nodes(
        &mut self,
        frame_size: Option<Size>,
        _pos: Point,
        children: &[TreeNode],
        list_level: usize,
        inherited_align: HAlignIr,
    ) -> Option<BlockNode> {
        // Check if this container starts with background/border `Shape` items (e.g. `#block(fill: ..., stroke: ...)[...]`)
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

        let mut max_w_pt = 0.0_f64;
        let mut max_h_pt = 0.0_f64;
        let mut main_fill = None;
        let mut left_accent_border: Option<StrokeIr> = None;

        let min_shape_x = bg_shapes
            .iter()
            .map(|(p, _, _)| p.x.to_pt())
            .reduce(f64::min)
            .unwrap_or(0.0);

        for (spos, s, _) in &bg_shapes {
            let b = s.geometry.bbox(None);
            let sw = b.size().x.to_pt().abs();
            let sh = b.size().y.to_pt().abs();
            if sw > max_w_pt {
                max_w_pt = sw;
                if let Some(f) = s.fill.as_ref().and_then(paint_to_hex) {
                    main_fill = Some(f);
                }
            }
            if sh > max_h_pt {
                max_h_pt = sh;
            }
            if (0.5..=12.0).contains(&sw)
                && sh >= 12.0
                && (spos.x.to_pt() - min_shape_x).abs() <= 4.0
                && let Some(c) = s.fill.as_ref().and_then(paint_to_hex)
            {
                left_accent_border = Some(StrokeIr {
                    color: c,
                    thickness_pt: sw,
                    dash: DashPatternIr::Solid,
                    ..Default::default()
                });
            }
        }

        let fill = main_fill.or_else(|| {
            bg_shapes
                .iter()
                .find_map(|(_, s, _)| s.fill.as_ref().and_then(paint_to_hex))
        });
        let stroke = bg_shapes
            .iter()
            .find_map(|(_, s, _)| s.stroke.as_ref().map(Self::fixed_stroke_to_ir));
        if fill.is_none() && stroke.is_none() && left_accent_border.is_none() {
            return None;
        }

        let mut line_starts = Vec::new();
        Self::collect_line_left_edges(&content_nodes, Point::zero(), &mut line_starts);
        let inferred_pad = line_starts
            .iter()
            .map(|(_, lx)| if min_shape_x > 10.0 { *lx - min_shape_x } else { *lx })
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

        let width_pt = frame_size
            .map(|sz| sz.x.to_pt())
            .unwrap_or(max_w_pt)
            .max(40.0);
        let height_pt = frame_size
            .map(|sz| sz.y.to_pt())
            .unwrap_or(max_h_pt)
            .max(20.0);
        let inset = SidesPtIr {
            top: v_pad,
            bottom: v_pad,
            left: inferred_pad,
            right: inferred_pad,
        };

        // If the styled `#block(stroke: ..., fill: ...)[...]` has a left-only accent border,
        // or contains tables, lists, enums, figures, rules, CeTZ canvases, or more than 3 blocks,
        // represent it as a breakable 1-cell bordered `BlockNode::Table` (`is_styled_container: true`).
        let needs_table_container = (left_accent_border.is_some() && stroke.is_none())
            || body_blocks.iter().any(|b| {
                !matches!(
                    b,
                    BlockNode::Paragraph { .. }
                        | BlockNode::Heading { .. }
                        | BlockNode::EquationBlock(_)
                        | BlockNode::CodeBlock { .. }
                )
            })
            || body_blocks.len() > 3;

        if needs_table_container {
            self.report.tables_count += 1;
            let cell_borders = if left_accent_border.is_some() && stroke.is_none() {
                BordersIr {
                    left: left_accent_border,
                    ..Default::default()
                }
            } else {
                BordersIr::splat(stroke)
            };
            return Some(BlockNode::Table(TableIr {
                kind: TableKindIr::StyledContainer,
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
                        borders: cell_borders,
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
                if rx > 0.5 && ry.abs() < 0.5 && rx <= width_pt * 0.5 {
                    return Some(rx);
                } else if ry > 0.5 && rx.abs() < 0.5 && ry <= height_pt * 0.5 {
                    return Some(ry);
                }
            }
            None
        });

        self.report.shapes_count += 1;
        Some(BlockNode::ShapeBlock(ShapeIr {
            kind: ShapeKindIr::Rect,
            width_pt,
            height_pt,
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
            if let TreeNode::Element { elem, .. } = node {
                if elem.is::<TableCell>() || elem.is::<GridCell>() {
                    return true;
                }
                if let Some(raw) = elem.to_packed::<RawElem>()
                    && raw.block.get(typst::foundations::StyleChain::default())
                {
                    return true;
                }
            }
        }
        false
    }
}
