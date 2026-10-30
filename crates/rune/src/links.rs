//! Pressable spans in text (`El::link`). Each link's line boxes come from the shaped layout,
//! so only its glyphs take the pointer. The text element holds focus for its links and
//! `Runtime::focused_link` says which one; links are reported to tools as their own elements.
use super::*;
use crate::text::{Link, LinkPaint};

#[derive(Default)]
pub(super) struct LinkState {
    /// The link a pointer press began on: (text path, link index).
    press: Option<(String, usize)>,
    /// The link a secondary press began on, in the captured text.
    secondary: Option<usize>,
}

pub(super) struct LinkElement<'a> {
    pub index: usize,
    pub key: &'a str,
    pub role: &'static str,
    pub label: Option<&'a str>,
    pub text: Option<&'a str>,
    pub rect: [f32; 4],
    pub visible: [f32; 4],
    pub focused: bool,
    pub hovered: bool,
    pub pressed: bool,
}

fn union(k: usize, boxes: &[(usize, [f32; 4])]) -> Option<[f32; 4]> {
    boxes.iter().filter(|(j, _)| *j == k).fold(None, |u: Option<[f32; 4]>, (_, r)| {
        Some(u.map_or(*r, |u| [u[0].min(r[0]), u[1].min(r[1]), u[2].max(r[2]), u[3].max(r[3])]))
    })
}

/// Where node `n`'s text starts inside its border box `rect` (points, x0 y0 x1 y1), its text
/// drawn `k` times its size.
pub(super) fn origin(n: &Node, spec: &TextSpec, layout: &Layout<Ink>, rect: [f32; 4], s: f32, k: f32) -> [f32; 2] {
    let x = text_x(spec, rect[0] + n.inset[0] * k, rect[2] - n.inset[2] * k, (layout.full_width() * k).ceil() / s);
    [x, rect[1] + n.inset[1] * k]
}

/// Link bytes `a..b` of node `n`'s text as bytes of its drawn layout, which truncation may
/// have cut (empty when the cut hides the link).
fn drawn(n: &Node, a: u32, b: u32) -> (u32, u32) {
    let (a, b) = n.elided.map_or((a as usize, b as usize), |e| e.mark(a as usize, b as usize, true));
    (a as u32, b as u32)
}

/// Reading order uses source offsets; the declaration index remains the link's identity.
fn link_order(index: usize, link: &Link) -> (u32, usize) { (link.start, index) }

/// A link must name authored text and retain drawn bytes before it can have geometry.
fn link_range(n: &Node, spec: &TextSpec, link: &Link) -> Option<(u32, u32)> {
    if link.start >= link.end
        || spec.content.get(link.start as usize..link.end as usize).is_none()
        || spec.source_slice(link.start, link.end).is_none_or(str::is_empty)
    {
        return None;
    }
    let (a, b) = drawn(n, link.start, link.end);
    (a < b && (b as usize) <= layout_len(n, spec)).then_some((a, b))
}

/// Tests whether the same range used by `boxes` produces a box, without collecting boxes.
fn link_has_box(n: &Node, spec: &TextSpec, link: &Link) -> bool {
    let Some(layout) = &n.text_layout else { return false };
    let Some((a, b)) = link_range(n, spec, link) else { return false };
    crate::text::range_has_boxes(layout, a as usize, b as usize)
}

/// The byte length of node `n`'s drawn text.
fn layout_len(n: &Node, spec: &TextSpec) -> usize {
    n.elided.map_or(spec.content.len(), |e| e.drawn_len(spec.content.len()))
}

/// Line boxes of bytes `start..end` in layout device px, with each box's line index.
fn boxes(layout: &Layout<Ink>, len: usize, start: u32, end: u32) -> Vec<(parley::BoundingBox, usize)> {
    let (a, b) = ((start as usize).min(len), (end as usize).min(len));
    crate::text::range_boxes(layout, a, b, false)
}

/// The link paints to draw this frame, by byte range, with the resting span's `bg_pad`.
pub(super) struct LinkFx(Vec<(u32, u32, LinkPaint, f32, bool)>);

impl LinkFx {
    pub(super) fn is_empty(&self) -> bool { self.0.is_empty() }

    pub(super) fn glyph_color(&self, byte: u32) -> Option<Color> {
        self.0.iter().find(|(a, b, p, _, _)| byte >= *a && byte < *b && p.color.is_some()).and_then(|(_, _, p, _, _)| p.color)
    }

    /// Backgrounds and underlines (device px, layout top-left at `origin`).
    pub(super) fn decos(&self, layout: &Layout<Ink>, len: usize, s: f32, origin: [f32; 2], out: &mut Vec<PaintedDeco>) {
        for &(a, b, p, bg_pad, has_rings) in &self.0 {
            for (bb, line) in boxes(layout, len, a, b) {
                let (x0, w) = (origin[0] + bb.x0 as f32, (bb.x1 - bb.x0) as f32);
                if p.bg.is_some() || p.ring.is_some() {
                    let pad = bg_pad * s;
                    let (mut y0, mut y1) = ((origin[1] + bb.y0 as f32).round(), (origin[1] + bb.y1 as f32).round());
                    if let Some(l) = layout.get(line) {
                        let base = origin[1] + l.metrics().baseline;
                        let (mut asc, mut desc) = (0.0f32, 0.0f32);
                        for run in l.runs() {
                            let r = run.text_range();
                            if r.start < b as usize && (a as usize) < r.end {
                                let fm = run.font_metrics();
                                (asc, desc) = (asc.max(fm.ascent), desc.max(fm.descent.abs()));
                            }
                        }
                        if asc > 0.0 || desc > 0.0 {
                            y0 = (base - asc - 1.0 * s).round();
                            y1 = (base + desc + 1.0 * s).round();
                        }
                    }
                    let rect = [x0 - pad, y0, w + 2.0 * pad, y1 - y0];
                    let ring = p.ring.map(|(c, rw)| (c, rw * s));
                    // A ring can split an otherwise uniform link background. Recolor those
                    // pieces in place, but don't mistake an isolated inline-code background
                    // for the whole link's hover background.
                    let contained = |d: &&PaintedDeco| {
                        d.radius > 0.0 && d.rect[0] >= rect[0] - 1.5
                            && d.rect[0] + d.rect[2] <= rect[0] + rect[2] + 1.5
                            && d.rect[1] < y1 && y0 < d.rect[1] + d.rect[3]
                    };
                    let split = has_rings && out.iter().filter(contained).any(|d| d.ring.is_some())
                        && out.iter().filter(contained).any(|d| (d.rect[0] - rect[0]).abs() < 1.5)
                        && out.iter().filter(contained).any(|d| (d.rect[0] + d.rect[2] - rect[0] - rect[2]).abs() < 1.5);
                    let mut matched = false;
                    for d in out.iter_mut().filter(|d| {
                        d.radius > 0.0
                            && d.rect[0] >= rect[0] - 1.5
                            && d.rect[0] + d.rect[2] <= rect[0] + rect[2] + 1.5
                            && (split || ((d.rect[0] - rect[0]).abs() < 1.5 && (d.rect[2] - rect[2]).abs() < 1.5))
                            && d.rect[1] < y1
                            && y0 < d.rect[1] + d.rect[3]
                    }) {
                        matched = true;
                        if let Some(c) = p.bg {
                            d.color = c;
                        }
                        if ring.is_some() {
                            d.ring = ring;
                        }
                    }
                    if !matched {
                        out.push(PaintedDeco { deco: Deco { rect, color: p.bg.unwrap_or(Color::TRANSPARENT), radius: 4.0 * s }, ring, byte: b.saturating_sub(1) });
                    }
                }
                if let (Some(c), Some(l)) = (p.underline, layout.get(line)) {
                    let y = (origin[1] + l.metrics().baseline + 2.0 * s).round();
                    if let Some(d) = out.iter_mut().find(|d| {
                        d.radius == 0.0
                            && (d.rect[0] - x0).abs() < 1.5
                            && (d.rect[2] - w).abs() < 1.5
                            && (d.rect[1] - y).abs() < 1.5
                    }) {
                        d.color = c;
                    } else {
                        out.push(PaintedDeco { deco: Deco { rect: [x0, y, w, s.max(1.0)], color: c, radius: 0.0 }, ring: None, byte: 0 });
                    }
                }
            }
        }
    }
}

impl<S: 'static> Runtime<S> {
    /// Node `i`'s border box as drawn this frame (points, x0 y0 x1 y1).
    pub(super) fn drawn_box(&self, i: usize) -> [f32; 4] {
        self.drawn.get(i).filter(|d| d[2] > 0.0 || d[3] > 0.0)
            .map_or(self.nodes[i].target, |d| [d[0], d[1], d[0] + d[2], d[1] + d[3]])
    }

    /// Node `i`'s link line boxes in window points (x0, y0, x1, y1) for border box `rect`.
    pub(super) fn link_boxes(&self, i: usize, rect: [f32; 4]) -> Vec<(usize, [f32; 4])> {
        let n = &self.nodes[i];
        let (Some(spec), Some(layout)) = (&n.text, &n.text_layout) else { return Vec::new() };
        let s = self.env.scale;
        let z = self.drawn_scale.get(i).copied().unwrap_or(1.0);
        let [ox, oy] = origin(n, spec, layout, rect, s, z);
        let px = s / z.max(1e-4);
        let mut out = Vec::new();
        for (k, l) in spec.links.iter().enumerate() {
            let Some((a, b)) = link_range(n, spec, l) else { continue };
            for (bb, _) in boxes(layout, layout_len(n, spec), a, b) {
                out.push((k, [ox + bb.x0 as f32 / px, oy + bb.y0 as f32 / px, ox + bb.x1 as f32 / px, oy + bb.y1 as f32 / px]));
            }
        }
        out
    }

    /// The link of node `i` under window point (x, y).
    pub(super) fn link_at(&self, i: usize, x: f32, y: f32) -> Option<usize> {
        if self.nodes[i].text.as_ref().is_none_or(|t| t.links.is_empty()) { return None; }
        let (rect, x, y) = self.unturned_box_and_point(i, x, y);
        self.link_boxes(i, rect).into_iter()
            .find(|(_, r)| contains(*r, x, y)).map(|(k, _)| k)
    }

    /// Hit rects for node `i`: only its links' boxes, clipped like `clipped`. False when it
    /// has no links, or is selectable (it takes its whole box).
    pub(super) fn push_link_hits(&mut self, i: usize, rect: [f32; 4], clipped: [f32; 4]) -> bool {
        if self.nodes[i].text.as_ref().is_none_or(|t| t.links.is_empty() || t.selectable) { return false; }
        let id = self.nodes[i].id;
        for (_, r) in self.link_boxes(i, rect) {
            let c = [r[0].max(clipped[0]), r[1].max(clipped[1]), r[2].min(clipped[2]), r[3].min(clipped[3])];
            if c[2] > c[0] && c[3] > c[1] { self.hits.push((id, c)); }
        }
        true
    }

    /// Which of node `i`'s links are hovered, pressed and keyboard-focused.
    pub(super) fn link_states(&self, i: usize) -> (Option<usize>, Option<usize>, Option<usize>) {
        let path = &self.nodes[i].path;
        let hovered = self.pointer.filter(|_| self.hovered.as_deref() == Some(path))
            .and_then(|(x, y)| self.link_at(i, x, y));
        let held = self.capture.as_ref().is_some_and(|(p, _)| p == path) && self.capture_inside;
        let pressed = self.links.press.as_ref().filter(|(p, k)| held && p == path && hovered == Some(*k)).map(|(_, k)| *k);
        let focused = self.focused_link.filter(|_| self.keyboard_focus && self.focused.as_deref() == Some(path));
        (hovered, pressed, focused)
    }

    /// The paints node `i`'s links show this frame.
    pub(super) fn link_fx(&self, i: usize) -> LinkFx {
        let Some(spec) = self.nodes[i].text.as_ref().filter(|t| !t.links.is_empty()) else { return LinkFx(Vec::new()) };
        let (hovered, pressed, focused) = self.link_states(i);
        let mut fx = Vec::new();
        for (k, l) in spec.links.iter().enumerate() {
            let paint = if pressed == Some(k) {
                l.pressed.unwrap_or(l.hover)
            } else if hovered == Some(k) {
                l.hover
            } else if focused == Some(k) {
                let p = l.focused.unwrap_or(l.hover);
                if p.is_empty() {
                    let ink = spec.spans.iter().filter(|sp| sp.start <= l.start && sp.end > l.start).find_map(|sp| sp.color);
                    LinkPaint::new().underline(ink.unwrap_or(spec.color))
                } else { p }
            } else { continue };
            let (a, b) = drawn(&self.nodes[i], l.start, l.end);
            let pad = spec.spans.iter().find(|sp| sp.start < l.end && sp.end > l.start && sp.bg.is_some()).map_or(2.0, |sp| sp.bg_pad);
            fx.push((a, b, paint, pad, !spec.rings.is_empty()));
        }
        LinkFx(fx)
    }

    /// Pointer down on node `i`: remember the link it began on.
    pub(super) fn link_down(&mut self, i: usize, x: f32, y: f32) -> bool {
        let link = self.link_at(i, x, y);
        self.links.press = link.map(|k| (self.nodes[i].path.clone(), k));
        link.is_some()
    }

    /// Pointer up: a press that began and ends on the same link activates it.
    pub(super) fn link_up(&mut self, path: &str, x: f32, y: f32) {
        let Some((p, k)) = self.links.press.take() else { return };
        let Some(i) = self.nodes.iter().position(|n| n.path == p) else { return };
        if p == path && self.capture_inside && self.link_at(i, x, y) == Some(k) {
            self.queue.push(Action::Link(p, k));
        }
    }

    /// Runs the `on_link` handler of node `path` for link `k`.
    pub(super) fn press_link(&mut self, path: &str, k: usize, tx: &mut Tx) -> bool {
        let Some((f, id)) = self.node(path).and_then(|n| {
            let t = n.text.as_ref()?;
            Some((t.on_link.clone()?, t.links.get(k)?.id.clone()))
        }) else { return false };
        log_event!(self, "link {path} {id}");
        (f.0)(states!(self), &id, tx);
        true
    }

    /// Node `i`'s link under (x, y), if its text takes secondary presses on links.
    pub(super) fn secondary_link_at(&self, i: usize, x: f32, y: f32) -> Option<usize> {
        self.nodes[i].text.as_ref()?.on_link_secondary.as_ref()?;
        self.link_at(i, x, y)
    }

    /// Secondary down on captured node `i`: remember the link it began on.
    pub(super) fn secondary_link_down(&mut self, i: Option<usize>, x: f32, y: f32) {
        self.links.secondary = i.and_then(|i| self.secondary_link_at(i, x, y));
    }

    pub(super) fn take_secondary_link(&mut self) -> Option<usize> { self.links.secondary.take() }

    /// Runs the `on_link_secondary` handler of node `path` for link `k`.
    pub(super) fn secondary_press_link(&mut self, path: &str, k: usize, point: PointerEvent, tx: &mut Tx) -> bool {
        let Some((f, id)) = self.node(path).and_then(|n| {
            let t = n.text.as_ref()?;
            Some((t.on_link_secondary.clone()?, t.links.get(k)?.id.clone()))
        }) else { return false };
        log_event!(self, "link secondary {path} {id}");
        (f.0)(states!(self), &id, point.window(), tx);
        true
    }

    /// Centre of the first drawn line box of the link whose id is `id`, in points.
    pub(super) fn find_link(&self, id: &str) -> Option<(f32, f32)> {
        (0..self.nodes.len()).filter(|&i| !self.subtree_exiting(i)).find_map(|i| {
            let k = self.nodes[i].text.as_ref()?.links.iter().position(|l| l.id == id)?;
            let (_, r) = self.link_boxes(i, self.drawn_box(i)).into_iter().find(|(j, _)| *j == k)?;
            Some(((r[0] + r[2]) * 0.5, (r[1] + r[3]) * 0.5))
        })
    }

    /// Finds the nearest eligible link in `(source start, declaration index)` order.
    /// Each scan uses constant space and, in the worst case, inspects the layout for
    /// every link. This avoids a retained ordering cache and allocations on Tab.
    fn adjacent_link(&self, i: usize, from: Option<usize>, backward: bool) -> Option<usize> {
        let n = &self.nodes[i];
        let spec = n.text.as_ref()?;
        let edge = match from {
            Some(k) => Some(link_order(k, spec.links.get(k)?)),
            None => None,
        };
        let mut best = None;
        for (k, link) in spec.links.iter().enumerate() {
            let key = link_order(k, link);
            if edge.is_some_and(|edge| if backward { key >= edge } else { key <= edge })
                || best.is_some_and(|best| if backward { key <= best } else { key >= best })
            {
                continue;
            }
            if link_has_box(n, spec, link) { best = Some(key); }
        }
        best.map(|(_, k)| k)
    }

    pub(super) fn has_focusable_link(&self, i: usize) -> bool {
        let n = &self.nodes[i];
        n.text.as_ref().is_some_and(|spec| spec.links.iter().any(|link| link_has_box(n, spec, link)))
    }

    pub(super) fn link_eligible(&self, i: usize, k: usize) -> bool {
        let n = &self.nodes[i];
        n.text.as_ref().is_some_and(|spec| spec.links.get(k).is_some_and(|link| link_has_box(n, spec, link)))
    }

    pub(super) fn link_focus_target(&self, i: usize) -> Option<usize> {
        self.focused_link_index(i).or_else(|| self.adjacent_link(i, None, false))
    }

    /// Tab inside the focused text's links, or return false at its edge.
    pub(super) fn link_tab(&mut self, backward: bool) -> bool {
        let Some(i) = self.focused_index() else { return false };
        let Some(k) = self.focused_link_index(i) else { return false };
        let next = self.adjacent_link(i, Some(k), backward);
        if next.is_some() { self.focused_link = next; }
        next.is_some()
    }

    /// Tab focused node `i`: start at its first link, or its last going backward.
    pub(super) fn enter_links(&mut self, i: usize, backward: bool) {
        self.focused_link = self.adjacent_link(i, None, backward);
    }

    /// Whether focused node `i` has a link focused, which takes Return.
    pub(super) fn link_focused(&self, i: usize) -> bool {
        self.focused_link.is_some_and(|k| self.link_eligible(i, k))
    }

    /// The id of focused node `i`'s focused link, which tools list as its own element.
    pub(super) fn focused_link_id(&self, i: usize) -> Option<String> {
        let k = self.focused_link_index(i)?;
        self.nodes[i].text.as_ref()?.links.get(k).map(|l| l.id.clone())
    }

    pub(super) fn focused_link_index(&self, i: usize) -> Option<usize> {
        self.focused_link.filter(|&k| self.focused.as_deref() == Some(&self.nodes[i].path) && self.link_eligible(i, k))
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub(super) fn focus_link(&mut self, i: usize, k: usize) -> bool {
        if !self.can_focus(i) || !self.link_eligible(i, k) { return false; }
        let changed = self.set_focus(Some(self.nodes[i].path.clone()), true) || self.focused_link != Some(k);
        self.focused_link = Some(k);
        self.dirty |= changed;
        self.pending.scroll_to.push((None, self.nodes[i].path.clone()));
        true
    }

    /// The Menu key on focused node `i`'s focused link: its `on_link_secondary`, at the
    /// bottom-left of its first line box.
    pub(super) fn focused_link_menu(&mut self, i: usize, tx: &mut Tx) -> bool {
        let Some(k) = self.focused_link_index(i) else { return false };
        if self.nodes[i].text.as_ref().is_none_or(|t| t.on_link_secondary.is_none()) { return false; }
        let Some((_, r)) = self.link_boxes(i, self.drawn_box(i)).into_iter().find(|(j, _)| *j == k) else { return false };
        let path = self.nodes[i].path.clone();
        let point = self.pointer_event(i, r[0], r[3]);
        self.secondary_press_link(&path, k, point, tx)
    }

    /// Enter or Space on a focused link presses it.
    pub(super) fn link_key(&mut self, i: usize, tx: &mut Tx) -> bool {
        let Some(k) = self.focused_link_index(i) else { return false };
        let path = self.nodes[i].path.clone();
        self.press_link(&path, k, tx);
        true
    }

    pub(super) fn visit_link_elements<'a>(&'a self, i: usize, visible: [f32; 4], mut visit: impl FnMut(LinkElement<'a>)) {
        let n = &self.nodes[i];
        let Some(spec) = n.text.as_ref().filter(|t| !t.links.is_empty()) else { return };
        let mut boxes = self.link_boxes(i, self.drawn_box(i));
        boxes.sort_unstable_by_key(|(k, _)| link_order(*k, &spec.links[*k]));
        let (hovered, pressed, _) = self.link_states(i);
        let focused = self.focused_link_index(i);
        let mut boxes = boxes.into_iter().peekable();
        while let Some((k, mut r)) = boxes.next() {
            while boxes.peek().is_some_and(|(next, _)| *next == k) {
                let (_, part) = boxes.next().expect("the next link box was just observed");
                r = [r[0].min(part[0]), r[1].min(part[1]), r[2].max(part[2]), r[3].max(part[3])];
            }
            let link = &spec.links[k];
            let (x0, y0) = (r[0].max(visible[0]), r[1].max(visible[1]));
            let (x1, y1) = (r[2].min(visible[0] + visible[2]), r[3].min(visible[1] + visible[3]));
            let text = (!n.is_secure()).then(|| spec.source_slice(link.start, link.end)).flatten();
            visit(LinkElement {
                index: k, key: &link.id, role: link.role, label: link.label.as_deref().or(text), text,
                rect: [r[0], r[1], r[2] - r[0], r[3] - r[1]],
                visible: [x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)],
                focused: focused == Some(k), hovered: hovered == Some(k), pressed: pressed == Some(k),
            });
        }
    }

    /// Reports each link as its own element after its text, which hands it its focus.
    pub(super) fn link_elements(&self, elements: &mut Vec<ElementInfo>) {
        for i in 0..self.nodes.len() {
            let n = &self.nodes[i];
            if n.text.as_ref().is_none_or(|t| t.links.is_empty()) { continue; }
            let Some(at) = elements.iter().position(|e| e.path == n.path) else { continue };
            let text = elements[at].clone();
            elements[at].focusable = false;
            elements[at].focused = false;
            elements[at].focus_visible = false;
            let targets = self.link_boxes(i, n.target);
            self.visit_link_elements(i, text.visible, |link| {
                let t = union(link.index, &targets).map(|r| [r[0], r[1], r[2] - r[0], r[3] - r[1]]).unwrap_or(link.rect);
                elements.push(ElementInfo {
                    stuck: 0.0,
                    path: format!("{}#link-{}", n.path, link.index),
                    key: Some(link.key.to_owned()),
                    role: link.role,
                    label: link.label.map(str::to_owned),
                    text: link.text.map(str::to_owned),
                    rect: link.rect,
                    target: t,
                    visible: link.visible,
                    focusable: text.focusable,
                    focused: link.focused,
                    focus_visible: self.keyboard_focus && link.focused,
                    hover: if link.hovered { 1.0 } else { 0.0 },
                    press: if link.pressed { 1.0 } else { 0.0 },
                    parent: Some(i),
                    caret: None,
                    selected: None,
                    level: None,
                    expanded: None,
                    item_selected: false,
                    ..text.clone()
                });
            });
        }
    }

    /// Node `i`'s highlight line boxes in window points (x0, y0, x1, y1) for border box `rect`.
    pub(super) fn highlight_boxes(&self, i: usize, rect: [f32; 4]) -> Vec<(usize, [f32; 4])> {
        self.highlight_boxes_in(&self.nodes, i, rect, self.drawn_scale.get(i).copied().unwrap_or(1.0))
    }

    pub(super) fn highlight_boxes_in(&self, nodes: &[Node], i: usize, rect: [f32; 4], z: f32) -> Vec<(usize, [f32; 4])> {
        let n = &nodes[i];
        let (Some(spec), Some(layout)) = (&n.text, &n.text_layout) else { return Vec::new() };
        if spec.highlights.is_empty() { return Vec::new(); }
        let s = self.env.scale;
        let [ox, oy] = origin(n, spec, layout, rect, s, z);
        let px = s / z.max(1e-4);
        let len = layout_len(n, spec);
        let mut out = Vec::new();
        for (k, h) in spec.highlights.iter().enumerate() {
            let (a, b) = drawn(n, h.start, h.end);
            for (bb, _) in boxes(layout, len, a, b) {
                out.push((k, [ox + bb.x0 as f32 / px, oy + bb.y0 as f32 / px, ox + bb.x1 as f32 / px, oy + bb.y1 as f32 / px]));
            }
        }
        out
    }

    /// Unscrolled target rect `[x0, y0, x1, y1]` (in window points) and first node index for
    /// the highlight keyed `key` (or scoped `path/key`).
    pub(super) fn highlight_target_in(&self, nodes: &[Node], scope: Option<&str>, key: &str) -> Option<(usize, [f32; 4])> {
        let mut first_node = None;
        let mut union: Option<[f32; 4]> = None;
        for (i, n) in nodes.iter().enumerate() {
            if !n.parent.is_none_or(|p| super::in_scope(&nodes[p].path, scope)) {
                continue;
            }
            let Some(spec) = n.text.as_ref().filter(|t| !t.highlights.is_empty()) else { continue };
            let matches: Vec<_> = spec.highlights.iter().map(|h| h.key.as_deref().is_some_and(|hkey| {
                crate::flight::selects(key, &format!("{}/{hkey}", n.path), Some(hkey))
            })).collect();
            if matches.iter().any(|m| *m) {
                for (k, r) in self.highlight_boxes_in(nodes, i, n.target, 1.0) {
                    if matches[k] {
                        first_node.get_or_insert(i);
                        union = Some(union.map_or(r, |u| [u[0].min(r[0]), u[1].min(r[1]), u[2].max(r[2]), u[3].max(r[3])]));
                    }
                }
            }
        }
        Some((first_node?, union?))
    }

    /// Centre of the first drawn line box of the highlight keyed `key`, in points.
    pub(super) fn find_highlight(&self, key: &str) -> Option<(f32, f32)> {
        (0..self.nodes.len()).filter(|&i| !self.subtree_exiting(i)).find_map(|i| {
            let n = &self.nodes[i];
            let k = n.text.as_ref()?.highlights.iter().position(|h| {
                h.key.as_deref().is_some_and(|hk| crate::flight::selects(key, &format!("{}/{hk}", n.path), Some(hk)))
            })?;
            let (_, r) = self.highlight_boxes(i, self.drawn_box(i)).into_iter().find(|(j, _)| *j == k)?;
            Some(((r[0] + r[2]) * 0.5, (r[1] + r[3]) * 0.5))
        })
    }

    /// Reports each text highlight as its own `"mark"` element after its text.
    pub(super) fn highlight_elements(&self, elements: &mut Vec<ElementInfo>) {
        if !self.nodes.iter().any(|n| n.text.as_ref().is_some_and(|t| !t.highlights.is_empty())) { return; }
        let text_elements: std::collections::HashMap<_, _> = elements.iter().enumerate().map(|(i, e)| (e.path.clone(), i)).collect();
        for i in 0..self.nodes.len() {
            let n = &self.nodes[i];
            let Some(spec) = n.text.as_ref().filter(|t| !t.highlights.is_empty()) else { continue };
            let Some(&at) = text_elements.get(&n.path) else { continue };
            let text = elements[at].clone();
            let boxes = self.highlight_boxes(i, self.drawn_box(i));
            let targets = self.highlight_boxes(i, n.target);
            let unions = |bs: &[(usize, [f32; 4])]| {
                let mut out = vec![None; spec.highlights.len()];
                for &(k, r) in bs {
                    out[k] = Some(out[k].map_or(r, |u: [f32; 4]| [u[0].min(r[0]), u[1].min(r[1]), u[2].max(r[2]), u[3].max(r[3])]));
                }
                out
            };
            let (boxes, targets) = (unions(&boxes), unions(&targets));
            for (k, h) in spec.highlights.iter().enumerate() {
                let Some(r) = boxes[k] else { continue };
                let t = targets[k].unwrap_or(r);
                let v = text.visible;
                let (x0, y0) = (r[0].max(v[0]), r[1].max(v[1]));
                let (x1, y1) = (r[2].min(v[0] + v[2]), r[3].min(v[1] + v[3]));
                let words = spec.source_slice(h.start, h.end).map(str::to_owned);
                elements.push(ElementInfo {
                    stuck: 0.0,
                    path: format!("{}#mark-{k}", n.path),
                    key: h.key.clone(),
                    role: "mark",
                    label: words.clone(),
                    text: words,
                    rect: [r[0], r[1], r[2] - r[0], r[3] - r[1]],
                    target: [t[0], t[1], t[2] - t[0], t[3] - t[1]],
                    visible: [x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)],
                    fill: h.bg,
                    text_color: h.fg.or(text.text_color),
                    radius: 4.0,
                    focusable: false,
                    focused: false,
                    focus_visible: false,
                    hover: 0.0,
                    press: 0.0,
                    parent: Some(i),
                    caret: None,
                    selected: None,
                    level: None,
                    expanded: None,
                    item_selected: false,
                    ..text.clone()
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Key, KeyEvent, Link, Mods, PillStyle, col, el, text};

    fn key<S: 'static>(rt: &mut Runtime<S>, key: Key, shift: bool) {
        rt.key(KeyEvent { key, mods: Mods { shift, ..Mods::default() } });
        rt.frame(0.0);
    }

    fn focused<S: 'static>(rt: &Runtime<S>) -> Option<String> {
        rt.elements().into_iter().find(|e| e.focused).and_then(|e| e.key)
    }

    fn button(key: &str) -> El {
        el().key(key).size(50.0, 24.0).focusable(true).role("button")
    }

    #[derive(Default)]
    struct Order { pilled: bool, activated: Vec<String> }

    fn order_view(s: &Order, _: &mut Cx<Order>) -> El {
        let mut prose = text("First Second Third").key("prose")
            .link(Link::new(13..18, "third"))
            .link(Link::new(0..5, "first-a"))
            .link(Link::new(6..12, "second"))
            .link(Link::new(0..3, "first-b"))
            .on_link(|s: &mut Order, id, _| s.activated.push(id.to_owned()));
        if s.pilled { prose = prose.pill(0..5, PillStyle::new()); }
        col().size(300.0, 150.0).child(button("before")).child(prose).child(button("after"))
    }

    #[test]
    fn links_are_visited_and_navigated_in_source_order_with_original_action_ids() {
        for pilled in [false, true] {
            let mut rt = Runtime::new(Order { pilled, ..Order::default() }, order_view, (300.0, 150.0), Env::new(true, 1.0));
            rt.set_tab_visits_controls(true);
            rt.frame(0.0);
            let links: Vec<_> = rt.elements().into_iter().filter(|e| e.path.contains("#link-"))
                .map(|e| (e.key.unwrap(), e.path)).collect();
            assert_eq!(links.iter().map(|(key, _)| key.as_str()).collect::<Vec<_>>(), ["first-a", "first-b", "second", "third"]);
            for ((_, path), index) in links.iter().zip([1, 3, 2, 0]) {
                assert!(path.ends_with(&format!("#link-{index}")));
            }

            key(&mut rt, Key::Tab, false);
            assert_eq!(focused(&rt).as_deref(), Some("before"));
            for (name, activation) in [
                ("first-a", Key::Enter), ("first-b", Key::Text(" ".into())),
                ("second", Key::Enter), ("third", Key::Text(" ".into())),
            ] {
                key(&mut rt, Key::Tab, false);
                assert_eq!(focused(&rt).as_deref(), Some(name));
                key(&mut rt, activation, false);
            }
            assert_eq!(rt.state.activated, ["first-a", "first-b", "second", "third"]);
            key(&mut rt, Key::Tab, false);
            assert_eq!(focused(&rt).as_deref(), Some("after"));
            for name in ["third", "second", "first-b", "first-a", "before"] {
                key(&mut rt, Key::Tab, true);
                assert_eq!(focused(&rt).as_deref(), Some(name));
            }
        }
    }

    #[test]
    fn focus_admission_matches_exposed_geometry_for_controls_and_newlines() {
        fn view(pilled: &bool, _: &mut Cx<bool>) -> El {
            let mut prose = text("A\u{200b}\nB").key("prose").w(150.0).wrap()
                .link(Link::new(4..5, "newline"))
                .link(Link::new(1..4, "zero-width"))
                .link(Link::new(0..1, "a"))
                .link(Link::new(5..6, "b"))
                .link(Link::new(6..6, "empty"));
            if *pilled { prose = prose.pill(0..1, PillStyle::new()); }
            col().size(200.0, 100.0).child(prose)
        }
        for pilled in [false, true] {
            let mut rt = Runtime::new(pilled, view, (200.0, 100.0), Env::new(true, 1.0));
            rt.frame(0.0);
            let prose = rt.nodes.iter().position(|n| n.key.as_deref() == Some("prose")).unwrap();
            let elements = rt.elements();
            for (index, name) in ["newline", "zero-width", "a", "b", "empty"].into_iter().enumerate() {
                let exposed = elements.iter().any(|e| e.key.as_deref() == Some(name) && e.path.ends_with(&format!("#link-{index}")));
                assert_eq!(rt.focus_link(prose, index), exposed, "{name}, pilled: {pilled}");
                if exposed {
                    rt.frame(0.0);
                    assert_eq!(focused(&rt).as_deref(), Some(name));
                }
            }
        }
    }

    #[test]
    fn empty_and_invalid_ranges_have_no_semantic_or_keyboard_target() {
        fn view(_: &(), _: &mut Cx<()>) -> El {
            let (start, end) = (8, 3);
            col().size(300.0, 180.0)
                .child(button("before"))
                .child(text("no links"))
                .child(text("empty").key("empty").link(Link::new(1..1, "empty-link")))
                .child(text("é alpha").key("prose")
                    .link(Link::new(1..2, "split-codepoint"))
                    .link(Link::new(20..25, "out-of-range"))
                    .link(Link::new(start..end, "reversed"))
                    .link(Link::new(3..8, "alpha"))
                    .link(Link::new(0..2, "accent")))
                .child(text("only").key("single").link(Link::new(0..4, "only-link")))
                .child(button("after"))
        }
        let mut rt = Runtime::new((), view, (300.0, 180.0), Env::new(true, 1.0));
        rt.set_tab_visits_controls(true);
        rt.frame(0.0);
        let links: Vec<_> = rt.elements().into_iter().filter(|e| e.path.contains("#link-"))
            .map(|e| e.key.unwrap()).collect();
        assert_eq!(links, ["accent", "alpha", "only-link"]);
        let empty = rt.nodes.iter().position(|n| n.key.as_deref() == Some("empty")).unwrap();
        let prose = rt.nodes.iter().position(|n| n.key.as_deref() == Some("prose")).unwrap();
        assert!(!rt.focus_link(empty, 0));
        for index in 0..3 { assert!(!rt.focus_link(prose, index)); }
        for name in ["before", "accent", "alpha", "only-link", "after"] {
            key(&mut rt, Key::Tab, false);
            assert_eq!(focused(&rt).as_deref(), Some(name));
        }
        for name in ["only-link", "alpha", "accent", "before"] {
            key(&mut rt, Key::Tab, true);
            assert_eq!(focused(&rt).as_deref(), Some(name));
        }
    }

    #[test]
    fn fully_elided_links_cannot_take_focus() {
        fn view(_: &(), _: &mut Cx<()>) -> El {
            col().size(300.0, 100.0).child(button("before"))
                .child(text("A long line with hidden suffix").key("prose").w(55.0).truncate()
                    .link(Link::new(22..28, "hidden"))
                    .link(Link::new(0..1, "shown")))
                .child(button("after"))
        }
        let mut rt = Runtime::new((), view, (300.0, 100.0), Env::new(true, 1.0));
        rt.set_tab_visits_controls(true);
        rt.frame(0.0);
        let links: Vec<_> = rt.elements().into_iter().filter(|e| e.path.contains("#link-"))
            .map(|e| e.key.unwrap()).collect();
        assert_eq!(links, ["shown"]);
        let prose = rt.nodes.iter().position(|n| n.key.as_deref() == Some("prose")).unwrap();
        assert!(!rt.focus_link(prose, 0));
        for name in ["before", "shown", "after"] {
            key(&mut rt, Key::Tab, false);
            assert_eq!(focused(&rt).as_deref(), Some(name));
        }
        key(&mut rt, Key::Tab, true);
        assert_eq!(focused(&rt).as_deref(), Some("shown"));
    }

    #[test]
    fn a_link_that_becomes_elided_cannot_keep_stale_focus() {
        struct View { narrow: bool, show_first: bool }
        fn view(s: &View, _: &mut Cx<View>) -> El {
            let mut prose = text("A long line with hidden suffix").key("prose")
                .w(if s.narrow { 55.0 } else { 250.0 }).truncate()
                .link(Link::new(22..28, "hidden"));
            if s.show_first { prose = prose.link(Link::new(0..1, "shown")); }
            col().size(300.0, 100.0).child(prose)
        }
        let mut rt = Runtime::new(View { narrow: false, show_first: true }, view, (300.0, 100.0), Env::new(true, 1.0));
        rt.set_tab_visits_controls(true);
        rt.frame(0.0);
        key(&mut rt, Key::Tab, false);
        assert_eq!(focused(&rt).as_deref(), Some("shown"));
        key(&mut rt, Key::Tab, false);
        assert_eq!(focused(&rt).as_deref(), Some("hidden"));
        rt.update(|s, _| s.narrow = true);
        rt.frame(0.0);
        assert_eq!(focused(&rt).as_deref(), Some("shown"));
        assert!(!rt.elements().iter().any(|e| e.key.as_deref() == Some("hidden")));
        rt.update(|s, _| s.show_first = false);
        rt.frame(0.0);
        assert_eq!(focused(&rt), None);
    }

    #[derive(Default)]
    struct Modal {
        open: bool,
        request_editor: bool,
        return_anchor: bool,
        editor: crate::TextEdit,
        activated: Vec<String>,
    }

    fn modal_view(s: &Modal, _: &mut Cx<Modal>) -> El {
        let opener = text("First Second").key("outside")
            .link(Link::new(6..12, "outside-second"))
            .link(Link::new(0..5, "outside-first"))
            .on_link(|s: &mut Modal, id, _| {
                s.activated.push(id.to_owned());
                s.open = true;
                s.editor.focused = s.request_editor;
            });
        let mut root = col().size(300.0, 200.0).child(opener);
        if s.open {
            let first = if s.request_editor {
                crate::text_input(&s.editor, crate::TextStyle::new(14.0, 400.0)).key("editor")
                    .size(100.0, 24.0).on_edit(|s: &mut Modal| &mut s.editor)
            } else {
                text("First Second").key("inside")
                    .link(Link::new(6..12, "inside-second"))
                    .link(Link::new(0..5, "inside-first"))
                    .on_link(|s: &mut Modal, id, _| s.activated.push(id.to_owned()))
            };
            let mut modal = col().key("modal").absolute(20.0, 40.0).size(200.0, 100.0)
                .modal(true, |s: &mut Modal| s.open = false);
            if s.return_anchor { modal = modal.focus_return("outside"); }
            root = root.child(modal.child(first).child(button("close").on_press(|s: &mut Modal| s.open = false)));
        }
        root
    }

    #[test]
    fn modal_entry_wrap_and_restore_keep_real_link_identity() {
        for (request_editor, return_anchor) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut rt = Runtime::new(Modal { request_editor, return_anchor, ..Modal::default() }, modal_view, (300.0, 200.0), Env::new(true, 1.0));
            rt.set_tab_visits_controls(true);
            rt.frame(0.0);
            key(&mut rt, Key::Tab, false);
            assert_eq!(focused(&rt).as_deref(), Some("outside-first"));
            key(&mut rt, Key::Tab, false);
            assert_eq!(focused(&rt).as_deref(), Some("outside-second"));
            key(&mut rt, Key::Enter, false);
            if request_editor {
                assert_eq!(focused(&rt).as_deref(), Some("editor"));
            } else {
                assert_eq!(focused(&rt).as_deref(), Some("inside-first"));
                for (shift, name) in [(true, "close"), (false, "inside-first"), (false, "inside-second")] {
                    key(&mut rt, Key::Tab, shift);
                    assert_eq!(focused(&rt).as_deref(), Some(name));
                }
                key(&mut rt, Key::Enter, false);
                assert_eq!(rt.state.activated, ["outside-second", "inside-second"]);
                for (shift, name) in [(false, "close"), (false, "inside-first"), (true, "close")] {
                    key(&mut rt, Key::Tab, shift);
                    assert_eq!(focused(&rt).as_deref(), Some(name));
                }
            }
            key(&mut rt, Key::Escape, false);
            assert!(!rt.state.open);
            assert_eq!(focused(&rt).as_deref(), Some("outside-second"));
        }
    }

    #[test]
    fn nested_modals_restore_the_link_focused_at_each_level() {
        #[derive(Default)]
        struct Nested { depth: u8 }
        fn view(s: &Nested, _: &mut Cx<Nested>) -> El {
            let outside = text("First Second").key("outside")
                .link(Link::new(6..12, "outside-second"))
                .link(Link::new(0..5, "outside-first"))
                .on_link(|s: &mut Nested, _, _| s.depth = 1);
            let mut root = col().size(300.0, 200.0).child(outside);
            if s.depth >= 1 {
                root = root.child(col().key("outer-modal").absolute(20.0, 40.0).size(200.0, 100.0)
                    .modal(true, |s: &mut Nested| s.depth = 0)
                    .child(text("First Second").key("outer-text")
                        .link(Link::new(6..12, "outer-second"))
                        .link(Link::new(0..5, "outer-first"))
                        .on_link(|s: &mut Nested, _, _| s.depth = 2)));
            }
            if s.depth >= 2 {
                root = root.child(col().key("inner-modal").absolute(40.0, 60.0).size(100.0, 50.0)
                    .modal(true, |s: &mut Nested| s.depth = 1).child(button("inner")));
            }
            root
        }
        let mut rt = Runtime::new(Nested::default(), view, (300.0, 200.0), Env::new(true, 1.0));
        rt.set_tab_visits_controls(true);
        rt.frame(0.0);
        key(&mut rt, Key::Tab, false);
        key(&mut rt, Key::Tab, false);
        assert_eq!(focused(&rt).as_deref(), Some("outside-second"));
        key(&mut rt, Key::Enter, false);
        assert_eq!(focused(&rt).as_deref(), Some("outer-first"));
        key(&mut rt, Key::Tab, false);
        assert_eq!(focused(&rt).as_deref(), Some("outer-second"));
        key(&mut rt, Key::Enter, false);
        assert_eq!(focused(&rt).as_deref(), Some("inner"));
        key(&mut rt, Key::Escape, false);
        assert_eq!(focused(&rt).as_deref(), Some("outer-second"));
        key(&mut rt, Key::Escape, false);
        assert_eq!(focused(&rt).as_deref(), Some("outside-second"));
    }
}
