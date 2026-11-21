//! Styled container groups (`#block(fill: ..., stroke: ...)[...]`) ->
//! semantic `BlockNode::Container`, 2-row `TableKindIr::StyledContainer`, or `BlockNode::ShapeBlock`.

use typst::introspection::Location;
use typst::layout::{GridCell, GroupItem, Point, Size};
use typst::model::TableCell;
use typst::pdf::ArtifactElem;
use typst::syntax::Span;
use typst::visualize::{CurveItem, Geometry};

use crate::color::paint_to_hex;
use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use ir::{
    BlockNode, BordersIr, ContainerIr, ContainerStyleIr, DashPatternIr, HAlignIr, SemanticRoleIr,
    ShapeIr, ShapeKindIr, SidesPtIr, StrokeIr, TableCellIr, TableIr, TableKindIr, TableRowIr,
    VAlignIr,
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
            Span::detached(),
            None,
            true,
            false,
            pos,
            children,
            list_level,
            inherited_align,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_extract_styled_container_nodes(
        &mut self,
        frame_size: Option<Size>,
        span: Span,
        loc_opt: Option<Location>,
        breakable: bool,
        sticky: bool,
        _pos: Point,
        children: &[TreeNode],
        list_level: usize,
        inherited_align: HAlignIr,
    ) -> Option<BlockNode> {
        // Check if this container starts with background/border `Shape` items (e.g. `#block(fill: ..., stroke: ...)[...]`)
        // AND is not a raw table/grid cell or CeTZ canvas.
        if Self::is_direct_table_or_code_container(children)
            || Self::is_cetz_group_nodes(children)
            || Self::is_vector_canvas_nodes(children)
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
        let mut stroke = None;
        let mut top_border: Option<StrokeIr> = None;
        let mut bottom_border: Option<StrokeIr> = None;
        let mut left_accent_border: Option<StrokeIr> = None;
        let mut right_border: Option<StrokeIr> = None;

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
            }
            if sh > max_h_pt {
                max_h_pt = sh;
            }
            if sw > 12.0 && sh > 6.0 {
                if let Some(f) = s.fill.as_ref().and_then(paint_to_hex) {
                    main_fill = Some(f);
                }
                if stroke.is_none() {
                    stroke = s.stroke.as_ref().map(Self::fixed_stroke_to_ir);
                }
            } else if sw > 12.0 && sh <= 6.0 {
                let border_stroke = s
                    .stroke
                    .as_ref()
                    .map(Self::fixed_stroke_to_ir)
                    .or_else(|| {
                        s.fill.as_ref().and_then(paint_to_hex).map(|c| StrokeIr {
                            color: c,
                            thickness_pt: sh.max(0.5),
                            dash: DashPatternIr::Solid,
                            ..Default::default()
                        })
                    });
                if let Some(st) = border_stroke {
                    let sy = spos.y.to_pt() + b.min.y.to_pt();
                    if sy <= 3.0 {
                        top_border = Some(st);
                    } else {
                        bottom_border = Some(st);
                    }
                }
            } else if (0.3..=12.0).contains(&sw) && sh > 6.0 {
                let border_stroke = s
                    .stroke
                    .as_ref()
                    .map(Self::fixed_stroke_to_ir)
                    .or_else(|| {
                        s.fill.as_ref().and_then(paint_to_hex).map(|c| StrokeIr {
                            color: c,
                            thickness_pt: sw.max(0.5),
                            dash: DashPatternIr::Solid,
                            ..Default::default()
                        })
                    });
                if let Some(st) = border_stroke {
                    if (spos.x.to_pt() - min_shape_x).abs() <= 4.0 {
                        left_accent_border = Some(st);
                    } else {
                        right_border = Some(st);
                    }
                }
            }
        }

        let fill = main_fill;
        let has_partial_border = top_border.is_some()
            || bottom_border.is_some()
            || left_accent_border.is_some()
            || right_border.is_some();
        if fill.is_none() && stroke.is_none() && !has_partial_border {
            return None;
        }

        let has_box_sides = fill.is_some()
            || stroke.is_some()
            || left_accent_border.is_some()
            || right_border.is_some();

        let mut line_starts = Vec::new();
        Self::collect_line_left_edges(&content_nodes, Point::zero(), &mut line_starts);
        Self::collect_shape_left_edges(&content_nodes, Point::zero(), &mut line_starts);
        // Preserve genuine `0pt` left insets (e.g. `#block(fill: luma(230))[...]`) rather than
        // filtering out `< 2.0pt` lines and falling back to an indented first line or a synthetic pad.
        let raw_min_left = line_starts
            .iter()
            .map(|(_, lx)| if min_shape_x > 10.0 { *lx - min_shape_x } else { *lx })
            .filter(|&lx| lx >= -0.5)
            .reduce(f64::min);
        let inferred_pad = if has_box_sides {
            match raw_min_left {
                Some(lx) if lx < 2.0 => 0.0,
                Some(lx) => lx.clamp(2.0, 24.0),
                None => 0.0,
            }
        } else {
            0.0
        };
        let v_pad = if has_box_sides {
            if inferred_pad < 2.0 {
                0.0
            } else {
                inferred_pad.clamp(4.0, 12.0)
            }
        } else {
            2.0
        };

        let width_pt = frame_size
            .map(|sz| sz.x.to_pt())
            .unwrap_or(max_w_pt)
            .max(40.0);
        let height_pt = frame_size
            .map(|sz| sz.y.to_pt())
            .unwrap_or(max_h_pt)
            .max(20.0);

        // Do not assume vertical padding equals horizontal `inset.x` when the laid-out height has
        // near-zero vertical margin around `content_h` (e.g. `#block(height: 100%, inset: (x: 1em))`).
        // `estimate_nodes_bbox` includes full glyph ascender/descender (~5.8pt beyond Typst's
        // `cap-height..baseline` text box), so only clamp when `height_pt <= content_h + 6.0`.
        let (_, _, _, content_h) = Self::estimate_nodes_bbox(&content_nodes);
        let v_pad = if has_box_sides && content_h > 0.5 && height_pt <= content_h + 6.0 {
            ((height_pt - content_h).max(0.0) * 0.5).min(v_pad)
        } else {
            v_pad
        };

        let has_full_width_child_group = content_nodes.iter().any(|n| {
            let mut cur = n;
            while let TreeNode::Element { children, .. } = cur {
                if children.len() == 1 {
                    cur = &children[0];
                } else {
                    break;
                }
            }
            matches!(
                cur,
                TreeNode::Group { group, pos, .. }
                    if pos.x.to_pt().abs() <= 2.0
                        && (group.frame.size().x.to_pt() - width_pt).abs() <= 4.0
            )
        });
        let id = self.alloc_node_with_fragments(
            span,
            loc_opt,
            Some(SemanticRoleIr::Container),
            children,
        );
        let mut body_blocks = self.with_scope(|ctx| {
            if !has_full_width_child_group && has_box_sides {
                ctx.scope.container_inset_pt += 2.0 * inferred_pad + 3.0;
            }
            ctx.scope.active_group_bounds_pt = Some((0.0, width_pt));
            ctx.parent_node_stack.push(id);
            let blocks =
                ctx.extract_blocks_from_nodes(&content_nodes, list_level, inherited_align);
            ctx.parent_node_stack.pop();
            blocks
        });

        if body_blocks.is_empty() {
            return None;
        }

        // When all body text blocks are horizontally centered, `line_starts`'s `min_x` reflects
        // centering whitespace `(width - text_w) / 2`, NOT box padding. Similarly, when every
        // paragraph is single-line with a global `first_line_indent_pt`, `raw_min_left` equals
        // `actual_inset + first_line_indent_pt`, so subtract `first_line_indent_pt` to avoid
        // double-counting first-line indent as box padding.
        let all_body_centered = !body_blocks.is_empty()
            && body_blocks.iter().all(|b| match b {
                BlockNode::Paragraph { style, .. } => style.align == HAlignIr::Center,
                BlockNode::Heading { align, .. } | BlockNode::Title { align, .. } => {
                    *align == HAlignIr::Center
                }
                _ => false,
            });
        let para_blocks: Vec<f64> = body_blocks
            .iter()
            .filter_map(|b| match b {
                BlockNode::Paragraph { style, .. } => Some(style.first_line_indent_pt),
                _ => None,
            })
            .collect();
        let single_line_first_indent = if !all_body_centered
            && !para_blocks.is_empty()
            && line_starts.len() <= para_blocks.len()
            && para_blocks.iter().all(|&fl| fl > 0.5)
        {
            para_blocks.into_iter().reduce(f64::min).unwrap_or(0.0)
        } else {
            0.0
        };
        let horiz_pad = if all_body_centered {
            inferred_pad.min(2.0)
        } else if single_line_first_indent > 0.5 {
            raw_min_left
                .map(|lx| (lx - single_line_first_indent).max(0.0).min(24.0))
                .unwrap_or(0.0)
        } else {
            inferred_pad
        };
        let eff_v_pad = if single_line_first_indent > 0.5 {
            if horiz_pad < 2.0 {
                0.0
            } else {
                v_pad.min(horiz_pad)
            }
        } else {
            v_pad
        };

        let inset = SidesPtIr {
            top: if has_box_sides || top_border.is_some() {
                eff_v_pad
            } else {
                0.0
            },
            bottom: if has_box_sides || bottom_border.is_some() {
                eff_v_pad
            } else {
                0.0
            },
            left: horiz_pad,
            right: horiz_pad,
        };

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

        // When a sharp-cornered container wraps only a single `CodeBlock` (`#block(fill: ...)[```...```]`),
        // transfer `(fill, stroke)` directly onto `CodeBlock` so it stays a native breakable code block.
        if body_blocks.len() == 1
            && corner_radius_pt.is_none()
            && !(has_partial_border && stroke.is_none())
            && let Some(BlockNode::CodeBlock {
                fill: code_fill,
                stroke: code_stroke,
                ..
            }) = body_blocks.first_mut()
        {
            if code_fill.is_none() {
                *code_fill = fill;
            }
            if code_stroke.is_none() {
                *code_stroke = stroke;
            }
            return body_blocks.pop();
        }

        let has_distinct_header_bar = match body_blocks.first() {
            Some(BlockNode::ShapeBlock(hdr)) => {
                hdr.fill.is_some() && hdr.fill != fill && (hdr.width_pt - width_pt).abs() <= 6.0
            }
            Some(BlockNode::Container(hdr_c)) => {
                hdr_c.style.fill.is_some() && hdr_c.style.fill != fill
            }
            Some(BlockNode::Table(hdr_tbl)) => {
                hdr_tbl.kind == TableKindIr::StyledContainer
                    && hdr_tbl.rows.len() == 1
                    && hdr_tbl.rows[0].cells.len() == 1
                    && hdr_tbl.rows[0].cells[0].fill.is_some()
                    && hdr_tbl.rows[0].cells[0].fill != fill
            }
            _ => false,
        };

        // Allow any rounded container fragment that fits within the page's drawable content area
        // (Typst already splits cross-page breakable `BlockElem`s into per-page `GroupItem` fragments).
        let max_single_page_box_h_pt = (self.current_page_content_height_pt() - 12.0).max(120.0);
        let can_use_rounded_shape_box = corner_radius_pt.is_some_and(|r| r > 0.5)
            && !(has_partial_border && stroke.is_none())
            && !has_distinct_header_bar
            && height_pt <= max_single_page_box_h_pt
            && body_blocks.iter().all(|b| {
                matches!(
                    b,
                    BlockNode::Paragraph { .. }
                        | BlockNode::Heading { .. }
                        | BlockNode::Title { .. }
                        | BlockNode::EquationBlock(_)
                        | BlockNode::Table(_)
                        | BlockNode::Container(_)
                        | BlockNode::List { .. }
                        | BlockNode::Enum { .. }
                        | BlockNode::CodeBlock { .. }
                        | BlockNode::UnsupportedBlock(_)
                )
            });

        // Sharp-cornered breakable `#block(fill/stroke)[...]` containers should lower to native
        // auto-height `BlockNode::Container` (`StyledContainer` `<w:tbl>`) so Word never clips
        // tall CJK/math lines inside fixed-height DrawingML textboxes and can break across pages.
        // Only rounded boxes (`corner_radius_pt > 0.5`) require DrawingML `roundRect`.
        let needs_table_container = !can_use_rounded_shape_box
            && (breakable
                || (has_partial_border && stroke.is_none())
                || body_blocks.iter().any(|b| {
                    !matches!(
                        b,
                        BlockNode::Paragraph { .. }
                            | BlockNode::Heading { .. }
                            | BlockNode::EquationBlock(_)
                    )
                })
                || body_blocks.len() > 3);

        if needs_table_container {
            let cell_borders = if has_partial_border && stroke.is_none() {
                BordersIr {
                    top: top_border,
                    bottom: bottom_border,
                    left: left_accent_border,
                    right: right_border,
                }
            } else {
                BordersIr {
                    top: top_border.or_else(|| stroke.clone()),
                    bottom: bottom_border.or_else(|| stroke.clone()),
                    left: left_accent_border.or_else(|| stroke.clone()),
                    right: right_border.or_else(|| stroke.clone()),
                }
            };

            // Detect 2-part card container (`#block(inset: 0pt, fill: F_body)[ #block(fill: F_hdr)[...] #block[...]]`)
            let header_bar = match body_blocks.first() {
                Some(BlockNode::ShapeBlock(hdr))
                    if hdr.fill.is_some()
                        && hdr.fill != fill
                        && (hdr.width_pt - width_pt).abs() <= 6.0 =>
                {
                    let Some(BlockNode::ShapeBlock(hdr)) = body_blocks.remove(0).into() else {
                        unreachable!()
                    };
                    Some((hdr.fill, hdr.inset, hdr.stroke, hdr.body))
                }
                Some(BlockNode::Container(hdr_c))
                    if hdr_c.style.fill.is_some() && hdr_c.style.fill != fill =>
                {
                    let BlockNode::Container(hdr_c) = body_blocks.remove(0) else {
                        unreachable!()
                    };
                    Some((
                        hdr_c.style.fill,
                        hdr_c.style.inset,
                        hdr_c.style.borders.bottom,
                        hdr_c.body,
                    ))
                }
                Some(BlockNode::Table(hdr_tbl))
                    if hdr_tbl.kind == TableKindIr::StyledContainer
                        && hdr_tbl.rows.len() == 1
                        && hdr_tbl.rows[0].cells.len() == 1
                        && hdr_tbl.rows[0].cells[0].fill.is_some()
                        && hdr_tbl.rows[0].cells[0].fill != fill =>
                {
                    let BlockNode::Table(mut hdr_tbl) = body_blocks.remove(0) else {
                        unreachable!()
                    };
                    let cell = hdr_tbl.rows.remove(0).cells.remove(0);
                    Some((cell.fill, cell.inset, cell.borders.bottom, cell.blocks))
                }
                _ => None,
            };

            if let Some((hdr_fill, hdr_inset, hdr_stroke, hdr_blocks)) = header_bar {
                self.report.tables_count += 1;
                let divider = hdr_stroke.or_else(|| stroke.clone());
                let hdr_cell = TableCellIr {
                    x: 0,
                    y: 0,
                    colspan: 1,
                    rowspan: 1,
                    fill: hdr_fill,
                    borders: BordersIr {
                        top: cell_borders.top.clone(),
                        left: cell_borders.left.clone(),
                        right: cell_borders.right.clone(),
                        bottom: divider.clone(),
                    },
                    inset: SidesPtIr {
                        top: 4.5,
                        bottom: 4.5,
                        left: hdr_inset.left.clamp(6.0, 12.0),
                        right: hdr_inset.right.clamp(6.0, 12.0),
                    },
                    halign: inherited_align,
                    valign: VAlignIr::Center,
                    is_header_cell: false,
                    blocks: hdr_blocks,
                };
                let mut rows = vec![TableRowIr {
                    height_pt: None,
                    is_header: false,
                    is_footer: false,
                    cells: vec![hdr_cell],
                }];
                if !body_blocks.is_empty() {
                    let body_cell = TableCellIr {
                        x: 0,
                        y: 1,
                        colspan: 1,
                        rowspan: 1,
                        fill,
                        borders: BordersIr {
                            top: divider,
                            left: cell_borders.left,
                            right: cell_borders.right,
                            bottom: cell_borders.bottom,
                        },
                        inset,
                        halign: inherited_align,
                        valign: VAlignIr::Top,
                        is_header_cell: false,
                        blocks: body_blocks,
                    };
                    rows.push(TableRowIr {
                        height_pt: None,
                        is_header: false,
                        is_footer: false,
                        cells: vec![body_cell],
                    });
                }
                let row_heights_pt = vec![0.0; rows.len()];
                return Some(BlockNode::Table(TableIr {
                    id,
                    kind: TableKindIr::StyledContainer,
                    cols: vec![ir::TableColTrackIr::Fixed { width_pt }],
                    row_heights_pt,
                    header_rows: 0,
                    footer_rows: 0,
                    rows,
                    align: inherited_align,
                    left_indent_pt: 0.0,
                    summary: None,
                }));
            }

            return Some(BlockNode::Container(ContainerIr {
                id,
                style: ContainerStyleIr {
                    fill,
                    borders: cell_borders,
                    corner_radius_pt,
                    inset,
                },
                width_pt: Some(width_pt),
                height_pt: Some(height_pt),
                align: inherited_align,
                left_indent_pt: 0.0,
                breakable,
                sticky,
                body: body_blocks,
            }));
        }

        self.report.shapes_count += 1;
        Some(BlockNode::ShapeBlock(ShapeIr {
            id,
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
            placed_offset_pt: None,
            baseline_shift_pt: None,
        }))
    }

    pub(crate) fn is_direct_table_or_code_container(nodes: &[TreeNode]) -> bool {
        // Only skip raw `TableCell` / `GridCell` wrappers; styled `#block(...)` containers wrapping
        // `RawElem` (e.g. macOS-style `#coder` cards) must be extracted as containers so their
        // background shape is not left behind as a separate empty `ShapeBlock`.
        for node in nodes {
            if let TreeNode::Element { elem, .. } = node
                && (elem.is::<TableCell>() || elem.is::<GridCell>())
            {
                return true;
            }
        }
        false
    }

    fn collect_shape_left_edges(
        nodes: &[TreeNode],
        offset: Point,
        lines: &mut Vec<(f64, f64)>,
    ) {
        for node in nodes {
            match node {
                TreeNode::Shape { pos, .. } => {
                    let x = (offset.x + pos.x).to_pt();
                    let y = (offset.y + pos.y).to_pt();
                    if x >= 2.0 {
                        if let Some(existing) = lines.iter_mut().find(|(ly, _)| (*ly - y).abs() < 3.0) {
                            existing.1 = existing.1.min(x);
                        } else {
                            lines.push((y, x));
                        }
                    }
                }
                TreeNode::Group { pos, children, .. } => {
                    Self::collect_shape_left_edges(children, offset + *pos, lines);
                }
                TreeNode::Element { children, .. } => {
                    Self::collect_shape_left_edges(children, offset, lines);
                }
                _ => {}
            }
        }
    }
}
