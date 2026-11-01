//! Column headers: the title, the sort, and the keyboard resize handle.
use std::rc::Rc;

use rune::{Color, Cursor, Drag, El, Key, Orientation, Phase, SortDirection, Tx, el, icon, row, text};

use super::{SortDir, TableColumn, TableState, TableStyle, Width, justify, sized};
use crate::Kit;

/// How far one arrow key moves a column, in points. Matches the pane grip.
const KEY_STEP: f32 = 8.0;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn header_row<S: 'static>(
    k: &Kit,
    state: &TableState,
    columns: &Rc<Vec<TableColumn>>,
    sizes: &Rc<Vec<Width>>,
    style: &TableStyle,
    on_sort: Option<Rc<dyn Fn(&mut S, &str, SortDir, &mut Tx)>>,
    get: fn(&S) -> &TableState,
    get_mut: fn(&mut S) -> &mut TableState,
) -> El {
    let _ = get;
    let mut h = row()
        .key("header")
        .role("row")
        .w_full()
        .h(style.header_height)
        .child(el().key("rule").pin(Some(0.0), None, Some(0.0), Some(0.0)).h(1.0).fill(style.rule));
    let last = columns.len().saturating_sub(1);
    for (j, c) in columns.iter().enumerate() {
        let sorted = state.sort().filter(|(id, _)| *id == c.id).map(|(_, d)| d);
        let title = text(c.title.clone())
            .key("title")
            .font(style.header_font)
            .color(if sorted.is_some() { style.text } else { style.header_text })
            .truncate()
            .shrink()
            .min_w(0.0)
            .help_when_truncated(c.title.clone());
        let mut inner = row().key("inner").gap(2.0).center_y().min_w(0.0).shrink().child(title);
        if let Some(d) = sorted {
            let glyph = if d == SortDir::Ascending { k.glyph.chevron_up } else { k.glyph.chevron_down };
            inner = inner.child(icon(glyph, 14.0).key("sort").color(style.text));
        }
        let mut cell = justify(
            sized(row().role("columnheader").label(c.title.clone()).h_full().px(style.cell_pad).center_y(), c, sizes[j]),
            c.align,
        )
        .key(c.id.clone())
        .sort(sorted.map(|d| match d {
            SortDir::Ascending => SortDirection::Ascending,
            SortDir::Descending => SortDirection::Descending,
        }))
        .child(inner);
        let id = c.id.clone();
        cell = cell.on_layout_tx(move |s: &mut S, r: [f32; 4], tx: &mut Tx| {
            if get_mut(s).laid_out.insert(id.clone(), r[2]) == Some(r[2]) { tx.live_only(); }
        });
        if c.sortable {
            let id = c.id.clone();
            let on_sort = on_sort.clone();
            let sort = Rc::new(move |s: &mut S, tx: &mut Tx| {
                let st = get_mut(s);
                let dir = st.next_sort(&id);
                st.set_sort(id.clone(), dir);
                if let Some(f) = &on_sort { f(s, &id, dir, tx); }
            });
            let press = sort.clone();
            let key = sort;
            cell = cell.focusable(true).cursor(Cursor::Pointer).hover(k.c.hover).on_press_tx(move |s: &mut S, tx: &mut Tx| press(s, tx)).on_key(
                move |s: &mut S, ev: &rune::KeyEvent, tx: &mut Tx| {
                    // Enter on the resize handle must not sort the column.
                    if ev.mods != rune::Mods::default() || ev.key != Key::Enter || tx.key_repeat() || tx.focused() == Some("resize") {
                        return false;
                    }
                    key(s, tx);
                    true
                },
            );
        }
        if c.resizable {
            cell = cell.child(resize_handle(c.clone(), state, get_mut, style.rule, j == last));
        } else if j != last {
            cell = cell.child(el().key("rule").pin(None, Some(8.0), Some(0.0), Some(8.0)).w(1.0).fill(style.rule));
        }
        h = h.child(cell);
    }
    h
}

/// The drag target on a header cell's right edge. Arrows step the width; a double click resets it.
fn resize_handle<S: 'static>(c: TableColumn, state: &TableState, get_mut: fn(&mut S) -> &mut TableState, rule: Color, last: bool) -> El {
    let id = c.id.clone();
    let reset = c.id.clone();
    let step = c.clone();
    let limit = c.clone();
    let published_max = if c.max.is_finite() { c.max.max(c.min) } else { 10_000.0_f32.max(c.min) };
    let width = state.column_width(&c.id).or_else(|| state.laid_out.get(&c.id).copied()).unwrap_or(c.min);
    let line = el().key("line").w(1.0).h_full().fill(if last { Color::TRANSPARENT } else { rule });
    let mut handle = row()
        .key("resize")
        .role("separator")
        .orientation(Orientation::Vertical)
        .label(format!("Resize {}", c.title));
    if c.min.is_finite() && published_max.is_finite() {
        let value = if width.is_finite() { width.clamp(c.min, published_max) } else { c.min };
        handle = handle.value_range(value, c.min..=published_max).value_text(format!("{value:.0} points"));
    }
    handle
        .pin(None, Some(6.0), Some(-4.0), Some(6.0))
        .w(9.0)
        .justify_center()
        .cursor(Cursor::ResizeColumn)
        .on_step(move |s: &mut S, delta, tx: &mut Tx| {
            tx.instant();
            let st = get_mut(s);
            let current = step.clamp(st.column_width(&step.id).or_else(|| st.laid_out.get(&step.id).copied()).unwrap_or(step.min));
            let max = if step.max.is_finite() { step.max.max(step.min) } else { 10_000.0_f32.max(step.min) };
            st.set_column_width(step.id.clone(), (current + delta as f32 * KEY_STEP).clamp(step.min, max));
        })
        .on_step_limit(move |s: &mut S, to_max, tx: &mut Tx| {
            tx.instant();
            let max = if limit.max.is_finite() { limit.max.max(limit.min) } else { 10_000.0_f32.max(limit.min) };
            get_mut(s).set_column_width(limit.id.clone(), if to_max { max } else { limit.min });
        })
        .on_drag(move |s: &mut S, d: &mut Drag, _tx: &mut Tx| {
            let st = get_mut(s);
            match d.phase {
                Phase::Down => {
                    st.resize_start = Some(st.column_width(&id));
                    d.data = st.column_width(&id).or_else(|| st.laid_out.get(&id).copied()).unwrap_or(c.min);
                }
                Phase::Move | Phase::Up => {
                    if d.cancelled() {
                        match st.resize_start.take().flatten() {
                            Some(w) => st.set_column_width(id.clone(), w),
                            None => st.reset_column_width(&id),
                        }
                    } else {
                        st.set_column_width(id.clone(), c.clamp(d.data + d.dx));
                        if d.phase == Phase::Up { st.resize_start = None; }
                    }
                }
            }
        })
        .on_double_press(move |s: &mut S| get_mut(s).reset_column_width(&reset))
        .child(line)
}
