//! LaTeX-style renderer for formula expressions.
//!
//! Renders a parsed [`Expr`] AST as structured math notation directly with the
//! egui painter (no LaTeX engine / extra dependencies): divisions with a
//! parenthesized operand become stacked fractions with a horizontal line,
//! `^` becomes a superscript, `sqrt`/`root` get a radical with overline, `abs`
//! gets vertical bars, and `log10`/`log2`/`log(x,b)` get subscript bases.
//!
//! Trace references are drawn in the color of the corresponding trace.
//!
//! Note: egui has no italic font family, so variables render upright; colors
//! distinguish trace references from surrounding text.

use std::collections::HashMap;

use eframe::egui::{self, Color32, FontId, Pos2, Stroke, Ui, Vec2};

use crate::data::expr::{BinOp, Expr, Func};
use crate::data::traces::TraceRef;

/// Fraction bar sits this fraction of the base font size above the baseline.
const MATH_AXIS: f32 = 0.28;
/// Vertical gap between a fraction bar and numerator/denominator.
const FRAC_GAP: f32 = 3.0;
/// Horizontal padding inside a fraction.
const FRAC_PAD_X: f32 = 4.0;
/// Font scale for super-/subscripts and radical indices.
const SCRIPT_SCALE: f32 = 0.7;
/// Vertical gap between a radical's overline and the radicand.
const RAD_GAP: f32 = 2.0;

/// Intermediate, unpositioned layout tree built from the AST.
enum FNode {
    Text {
        text: String,
        color: Color32,
    },
    Row {
        children: Vec<FNode>,
        gap: f32,
    },
    Frac {
        num: Box<FNode>,
        den: Box<FNode>,
    },
    Sup {
        base: Box<FNode>,
        sup: Box<FNode>,
    },
    Sub {
        base: Box<FNode>,
        sub: Box<FNode>,
    },
    Radical {
        inner: Box<FNode>,
        index: Option<Box<FNode>>,
    },
    Abs {
        inner: Box<FNode>,
    },
    Paren {
        inner: Box<FNode>,
    },
}

/// A laid-out node with computed metrics relative to its baseline.
struct LNode {
    kind: LKind,
    /// Total width.
    w: f32,
    /// Height above the baseline.
    asc: f32,
    /// Depth below the baseline.
    desc: f32,
}

enum LKind {
    Text {
        text: String,
        font: FontId,
        color: Color32,
    },
    Row {
        children: Vec<LNode>,
        gap: f32,
    },
    Frac {
        num: Box<LNode>,
        den: Box<LNode>,
        /// Distance of the fraction bar above the baseline.
        axis: f32,
    },
    Sup {
        base: Box<LNode>,
        sup: Box<LNode>,
        /// How far above the baseline the superscript's baseline sits.
        shift: f32,
    },
    Sub {
        base: Box<LNode>,
        sub: Box<LNode>,
        /// How far below the baseline the subscript's baseline sits.
        shift: f32,
    },
    Radical {
        inner: Box<LNode>,
        index: Option<Box<LNode>>,
        /// Width of the radical hook in front of the radicand.
        hook_w: f32,
        /// Distance of the overline above the baseline.
        over_y: f32,
    },
    Abs {
        inner: Box<LNode>,
        bar_w: f32,
    },
    Paren {
        inner: Box<LNode>,
        glyph_font: FontId,
        glyph_w: f32,
    },
}

/// Render `ast` as structured math in the given ui.
///
/// `colors` maps trace names to the color used for their reference; other
/// text uses `text_color`. Allocates exactly the space the formula needs and
/// returns the response (for tooltips etc.).
pub fn show_formula(
    ui: &mut Ui,
    ast: &Expr,
    colors: &HashMap<TraceRef, Color32>,
    text_color: Color32,
) -> egui::Response {
    let size = ui
        .style()
        .text_styles
        .get(&egui::TextStyle::Body)
        .map(|f| f.size)
        .unwrap_or(14.0);
    let node = to_fnode(ast, colors, text_color);
    let painter = ui.painter().clone();
    let laid = layout(&node, &painter, size);
    let desired = Vec2::new(laid.w.max(1.0), (laid.asc + laid.desc).max(1.0));
    let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let baseline = rect.top() + laid.asc;
    paint(&laid, &painter, rect.left(), baseline, text_color);
    resp
}

// =============================================================================
// AST -> layout tree
// =============================================================================

fn to_fnode(e: &Expr, colors: &HashMap<TraceRef, Color32>, text_color: Color32) -> FNode {
    let text = |s: &str| FNode::Text {
        text: s.to_string(),
        color: text_color,
    };
    match e {
        Expr::Num(v) => text(&fmt_num(*v)),
        Expr::Time => text("t"),
        Expr::Const(name, _) => text(match *name {
            "pi" => "π",
            other => other,
        }),
        Expr::Trace(name) => FNode::Text {
            text: name.0.clone(),
            color: colors.get(name).copied().unwrap_or(text_color),
        },
        Expr::Neg(inner) => FNode::Row {
            children: vec![text("−"), operand(inner, colors, text_color, true)],
            gap: 1.0,
        },
        Expr::Group(inner) => FNode::Paren {
            inner: Box::new(to_fnode(inner, colors, text_color)),
        },
        Expr::Bin(op, l, r) => match op {
            BinOp::Add => FNode::Row {
                children: vec![
                    to_fnode(l, colors, text_color),
                    text("+"),
                    to_fnode(r, colors, text_color),
                ],
                gap: 3.0,
            },
            BinOp::Sub => FNode::Row {
                children: vec![
                    to_fnode(l, colors, text_color),
                    text("−"),
                    // a-(b+c) must keep its parens
                    operand(r, colors, text_color, true),
                ],
                gap: 3.0,
            },
            BinOp::Mul => FNode::Row {
                children: vec![
                    operand(l, colors, text_color, true),
                    text("·"),
                    operand(r, colors, text_color, true),
                ],
                gap: 3.0,
            },
            BinOp::Pow => FNode::Sup {
                base: Box::new(operand(l, colors, text_color, true)),
                sup: Box::new(to_fnode(r, colors, text_color)),
            },
            BinOp::Div => {
                let frac = matches!(**l, Expr::Group(_)) || matches!(**r, Expr::Group(_));
                if frac {
                    FNode::Frac {
                        // Parentheses are dropped inside a stacked fraction.
                        num: Box::new(to_fnode(strip_group(l), colors, text_color)),
                        den: Box::new(to_fnode(strip_group(r), colors, text_color)),
                    }
                } else {
                    FNode::Row {
                        children: vec![
                            operand(l, colors, text_color, true),
                            text("⁄"),
                            operand(r, colors, text_color, true),
                        ],
                        gap: 3.0,
                    }
                }
            }
        },
        Expr::Call(f, args) => match f {
            Func::Sqrt => FNode::Radical {
                inner: Box::new(to_fnode(&args[0], colors, text_color)),
                index: None,
            },
            Func::Root => FNode::Radical {
                inner: Box::new(to_fnode(&args[0], colors, text_color)),
                index: Some(Box::new(to_fnode(&args[1], colors, text_color))),
            },
            Func::Abs => FNode::Abs {
                inner: Box::new(to_fnode(&args[0], colors, text_color)),
            },
            Func::Log10 => FNode::Row {
                children: vec![
                    FNode::Sub {
                        base: Box::new(text("log")),
                        sub: Box::new(FNode::Text {
                            text: "10".to_string(),
                            color: text_color,
                        }),
                    },
                    call_parens(args, colors, text_color),
                ],
                gap: 0.5,
            },
            Func::Log2 => FNode::Row {
                children: vec![
                    FNode::Sub {
                        base: Box::new(text("log")),
                        sub: Box::new(FNode::Text {
                            text: "2".to_string(),
                            color: text_color,
                        }),
                    },
                    call_parens(args, colors, text_color),
                ],
                gap: 0.5,
            },
            Func::Log if args.len() == 2 => FNode::Row {
                children: vec![
                    FNode::Sub {
                        base: Box::new(text("log")),
                        sub: Box::new(to_fnode(&args[1], colors, text_color)),
                    },
                    FNode::Paren {
                        inner: Box::new(to_fnode(&args[0], colors, text_color)),
                    },
                ],
                gap: 0.5,
            },
            _ => FNode::Row {
                children: vec![text(f.name()), call_parens(args, colors, text_color)],
                gap: 0.5,
            },
        },
    }
}

/// Render function-call arguments as `(a, b, …)`.
fn call_parens(args: &[Expr], colors: &HashMap<TraceRef, Color32>, text_color: Color32) -> FNode {
    let mut children = Vec::new();
    for (i, a) in args.iter().enumerate() {
        if i > 0 {
            children.push(FNode::Text {
                text: ",".to_string(),
                color: text_color,
            });
        }
        children.push(to_fnode(a, colors, text_color));
    }
    FNode::Paren {
        inner: Box::new(FNode::Row { children, gap: 3.0 }),
    }
}

/// Convert an operand, wrapping it in parentheses when `paren_if_complex` is
/// set and the expression is a composite that would read ambiguously without
/// them (binary ops, negation).
fn operand(
    e: &Expr,
    colors: &HashMap<TraceRef, Color32>,
    text_color: Color32,
    paren_if_complex: bool,
) -> FNode {
    let complex = matches!(e, Expr::Bin(..) | Expr::Neg(_));
    let node = to_fnode(e, colors, text_color);
    if paren_if_complex && complex {
        FNode::Paren {
            inner: Box::new(node),
        }
    } else {
        node
    }
}

fn strip_group(e: &Expr) -> &Expr {
    match e {
        Expr::Group(inner) => inner,
        other => other,
    }
}

fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

// =============================================================================
// Layout (bottom-up metric computation)
// =============================================================================

fn layout(n: &FNode, painter: &egui::Painter, size: f32) -> LNode {
    match n {
        FNode::Text { text, color } => {
            let font = FontId::proportional(size);
            let galley = painter.layout_no_wrap(text.clone(), font.clone(), *color);
            let h = galley.size().y;
            LNode {
                kind: LKind::Text {
                    text: text.clone(),
                    font,
                    color: *color,
                },
                w: galley.size().x,
                asc: h * 0.8,
                desc: h * 0.2,
            }
        }
        FNode::Row { children, gap } => {
            let laid: Vec<LNode> = children.iter().map(|c| layout(c, painter, size)).collect();
            let w =
                laid.iter().map(|c| c.w).sum::<f32>() + *gap * laid.len().saturating_sub(1) as f32;
            let asc = laid.iter().map(|c| c.asc).fold(0.0, f32::max);
            let desc = laid.iter().map(|c| c.desc).fold(0.0, f32::max);
            LNode {
                kind: LKind::Row {
                    children: laid,
                    gap: *gap,
                },
                w,
                asc,
                desc,
            }
        }
        FNode::Frac { num, den } => {
            let num = layout(num, painter, size);
            let den = layout(den, painter, size);
            let axis = MATH_AXIS * size;
            LNode {
                w: num.w.max(den.w) + 2.0 * FRAC_PAD_X,
                asc: axis + FRAC_GAP + num.asc + num.desc,
                desc: (FRAC_GAP + den.asc + den.desc - axis).max(0.0),
                kind: LKind::Frac {
                    num: Box::new(num),
                    den: Box::new(den),
                    axis,
                },
            }
        }
        FNode::Sup { base, sup } => {
            let base = layout(base, painter, size);
            let sup = layout(sup, painter, size * SCRIPT_SCALE);
            let shift = base.asc * 0.55;
            LNode {
                w: base.w + 1.0 + sup.w,
                asc: base.asc.max(shift + sup.asc),
                desc: base.desc,
                kind: LKind::Sup {
                    base: Box::new(base),
                    sup: Box::new(sup),
                    shift,
                },
            }
        }
        FNode::Sub { base, sub } => {
            let base = layout(base, painter, size);
            let sub = layout(sub, painter, size * SCRIPT_SCALE);
            let shift = base.desc + sub.asc * 0.5;
            LNode {
                w: base.w + sub.w,
                asc: base.asc,
                desc: base.desc.max(shift + sub.desc),
                kind: LKind::Sub {
                    base: Box::new(base),
                    sub: Box::new(sub),
                    shift,
                },
            }
        }
        FNode::Radical { inner, index } => {
            let inner = layout(inner, painter, size);
            let index = index
                .as_ref()
                .map(|i| layout(i, painter, size * SCRIPT_SCALE));
            let index_w = index.as_ref().map(|i| i.w + 1.0).unwrap_or(0.0);
            let hook_w = size * 0.45 + index_w;
            let over_y = inner.asc + RAD_GAP;
            LNode {
                w: hook_w + inner.w + 2.0,
                asc: over_y + 1.0,
                desc: inner.desc + 1.0,
                kind: LKind::Radical {
                    inner: Box::new(inner),
                    index: index.map(Box::new),
                    hook_w,
                    over_y,
                },
            }
        }
        FNode::Abs { inner } => {
            let inner = layout(inner, painter, size);
            let bar_w = 1.2;
            LNode {
                w: inner.w + 2.0 * (bar_w + 2.0),
                asc: inner.asc + 1.0,
                desc: inner.desc + 1.0,
                kind: LKind::Abs {
                    inner: Box::new(inner),
                    bar_w,
                },
            }
        }
        FNode::Paren { inner } => {
            let inner = layout(inner, painter, size);
            let inner_h = inner.asc + inner.desc;
            let glyph_size = (inner_h * 1.05).clamp(size * 0.95, size * 2.6);
            let glyph_font = FontId::proportional(glyph_size);
            let glyph_w = painter
                .layout_no_wrap("(".to_string(), glyph_font.clone(), Color32::WHITE)
                .size()
                .x;
            LNode {
                w: inner.w + 2.0 * (glyph_w + 2.0),
                asc: inner.asc + 1.0,
                desc: inner.desc + 1.0,
                kind: LKind::Paren {
                    inner: Box::new(inner),
                    glyph_font,
                    glyph_w,
                },
            }
        }
    }
}

// =============================================================================
// Painting
// =============================================================================

fn paint(n: &LNode, painter: &egui::Painter, x: f32, baseline: f32, text_color: Color32) {
    match &n.kind {
        LKind::Text { text, font, color } => {
            let galley = painter.layout_no_wrap(text.clone(), font.clone(), *color);
            painter.galley(Pos2::new(x, baseline - n.asc), galley, *color);
        }
        LKind::Row { children, gap } => {
            let mut cx = x;
            for c in children {
                paint(c, painter, cx, baseline, text_color);
                cx += c.w + gap;
            }
        }
        LKind::Frac { num, den, axis } => {
            let bar_y = baseline - axis;
            let num_x = x + (n.w - num.w) * 0.5;
            let den_x = x + (n.w - den.w) * 0.5;
            paint(num, painter, num_x, bar_y - FRAC_GAP - num.desc, text_color);
            paint(den, painter, den_x, bar_y + FRAC_GAP + den.asc, text_color);
            painter.line_segment(
                [Pos2::new(x, bar_y), Pos2::new(x + n.w, bar_y)],
                Stroke::new(1.0, text_color),
            );
        }
        LKind::Sup { base, sup, shift } => {
            paint(base, painter, x, baseline, text_color);
            paint(sup, painter, x + base.w + 1.0, baseline - shift, text_color);
        }
        LKind::Sub { base, sub, shift } => {
            paint(base, painter, x, baseline, text_color);
            paint(sub, painter, x + base.w, baseline + shift, text_color);
        }
        LKind::Radical {
            inner,
            index,
            hook_w,
            over_y,
        } => {
            let stroke = Stroke::new(1.2, text_color);
            let overline = baseline - over_y;
            let index_w = index.as_ref().map(|i| i.w + 1.0).unwrap_or(0.0);
            if let Some(idx) = index {
                paint(idx, painter, x, overline + idx.asc, text_color);
            }
            let hx = x + index_w;
            // Radical hook: small check mark, then up to the overline.
            painter.add(egui::Shape::line(
                vec![
                    Pos2::new(hx, baseline - n.asc * 0.35),
                    Pos2::new(hx + hook_w * 0.3, baseline + n.desc * 0.6),
                    Pos2::new(hx + hook_w * 0.62, overline - 1.0),
                ],
                stroke,
            ));
            // Overline above the radicand.
            painter.line_segment(
                [
                    Pos2::new(hx + hook_w * 0.62, overline - 1.0),
                    Pos2::new(x + n.w, overline - 1.0),
                ],
                stroke,
            );
            paint(inner, painter, x + hook_w, baseline, text_color);
        }
        LKind::Abs { inner, bar_w } => {
            let stroke = Stroke::new(*bar_w, text_color);
            let top = baseline - n.asc;
            let bottom = baseline + n.desc;
            painter.line_segment([Pos2::new(x, top), Pos2::new(x, bottom)], stroke);
            painter.line_segment(
                [
                    Pos2::new(x + n.w - *bar_w, top),
                    Pos2::new(x + n.w - *bar_w, bottom),
                ],
                stroke,
            );
            paint(inner, painter, x + bar_w + 2.0, baseline, text_color);
        }
        LKind::Paren {
            inner,
            glyph_font,
            glyph_w,
        } => {
            let center_y = baseline + (inner.desc - inner.asc) * 0.5;
            for (gx, glyph) in [(x, "("), (x + n.w - glyph_w - 1.0, ")")] {
                let galley =
                    painter.layout_no_wrap(glyph.to_string(), glyph_font.clone(), text_color);
                let gh = galley.size().y;
                painter.galley(Pos2::new(gx, center_y - gh * 0.5), galley, text_color);
            }
            paint(inner, painter, x + glyph_w + 2.0, baseline, text_color);
        }
    }
}
