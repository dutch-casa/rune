//! The retained observations shared by tooling and platform accessibility adapters.
use super::*;

pub(super) struct ElementGeometry<'a> {
    pub pose: rune_motion::Presented,
    pub flight: Option<&'a TextFlight>,
    pub rect: [f32; 4],
    pub visible: [f32; 4],
}

impl Node {
    pub(super) fn is_secure(&self) -> bool {
        self.a11y.secure || self.text.as_ref().is_some_and(|t| t.redacted)
            || self.input.as_ref().is_some_and(|i| i.edit.is_secure())
    }
}

impl<S: 'static> Runtime<S> {
    pub(super) fn element_geometry(&self, i: usize) -> Option<ElementGeometry<'_>> {
        let n = &self.nodes[i];
        let pose = self.scene.presented(n.id)?;
        let flight = self.flights.iter().find(|f| f.lands(&n.path, n.key.as_deref()));
        let drawn = self.drawn.get(i).copied().unwrap_or([pose.rect.x, pose.rect.y, pose.rect.w, pose.rect.h]);
        let rect = flight.map_or(drawn, |f| {
            let (d, c) = (f.destination, f.presented);
            let (zx, zy) = (c.w / d.w.max(1e-3), c.h / d.h.max(1e-3));
            [c.x + (drawn[0] - d.x) * zx, c.y + (drawn[1] - d.y) * zy, drawn[2] * zx, drawn[3] * zy]
        });
        let s = self.env.scale;
        let clip = if self.launches.flying(n.id) { NO_CLIP } else {
            flight.map_or_else(|| self.screen_clip_of_parent(n.parent), |f| f.clip.map(|v| v * s))
        };
        let (x0, y0) = (rect[0].max(clip[0] / s), rect[1].max(clip[1] / s));
        let (x1, y1) = ((rect[0] + rect[2]).min(clip[2] / s), (rect[1] + rect[3]).min(clip[3] / s));
        Some(ElementGeometry { pose, flight, rect, visible: [x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)] })
    }
    pub fn elements(&self) -> Vec<ElementInfo> {
        fn intent(n: &Node) -> u32 {
            n.live.iter().filter(|b| b.spring.is_none()).fold(0, |bits, b| bits | match b.property {
                crate::live::Property::Offset => 1,
                crate::live::Property::Scale | crate::live::Property::Rect => 1 | 2,
                crate::live::Property::Opacity => 4,
                crate::live::Property::Fill | crate::live::Property::Blur => 0,
            })
        }
        let store = self.scene.store();
        let animating = |id: NodeId| store.is_animating(id, prop::FRAME) || store.is_animating(id, prop::TRAVEL)
            || store.is_animating(id, prop::PRESENCE) || store.is_animating(id, prop::OFFSET) || store.is_animating(id, prop::SCALE) || store.is_animating(id, P_PRESS_SCALE)
            || store.is_animating(id, P_SCALE) || store.is_animating(id, prop::ROTATION) || paint_pose_springing(store, id)
            || (!self.reduced_motion && self.loops.get(&id).is_some_and(|l| l.active(store.node_time(id))));
        let sel_ranges = self.selection_ranges();
        let mut elements: Vec<_> = self.nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| {
                let ElementGeometry { pose: p, flight, rect, visible } = self.element_geometry(i)?;
                let exiting = self.subtree_exiting(i);
                let mut moving = flight.is_some() || animating(n.id) || self.grows.get(&n.id).is_some_and(|g| g.moving());
                let revealing = flight.is_some() || self.reveals.get(&n.id).is_some_and(|r| !r.words.is_empty());
                let mut in_growing_row = false;
                let mut scrolling = false;
                let mut live_intent = intent(n);
                // A group_opacity swap under an instant hover group is a deliberate cut (Beta's
                // status/close swap), for it and what it contains: opacity and transition intent.
                let up = |from: Option<usize>| std::iter::successors(from, |&pi| self.nodes[pi].parent);
                if let Some(gi) = up(Some(i)).find(|&j| self.nodes[j].vis.group_opacity.is_some()) {
                    let group = up(self.nodes[gi].parent)
                        .map(|pi| &self.nodes[pi].vis)
                        .find(|v| v.hover.a > 0.0 || v.hover_group);
                    if group.and_then(|v| v.hover_motion.as_deref().copied()).is_some_and(|(e, x)| [e, x].iter().any(|a| matches!(a.curve, rune_motion::Curve::Instant))) {
                        live_intent |= 4 | 16;
                    }
                }
                let step_up = |k: usize| {
                    if let Some(a) = &self.nodes[k].above
                        && let Some(j) = self.nodes[..k].iter().position(|n| n.key.as_ref() == Some(&a.key))
                    {
                        Some(j)
                    } else {
                        self.nodes[k].parent
                    }
                };
                let mut a = step_up(i);
                while let Some(pi) = a {
                    let parent = &self.nodes[pi];
                    moving |= animating(parent.id)
                        || self.stacks.get(&parent.id).is_some_and(|s| s.moving)
                        || self.scrolls.get(&parent.id).is_some_and(|s| s.slack.moving() || s.glide.is_some());
                    live_intent |= intent(parent);
                    scrolling |= self.scrolls.get(&parent.id).is_some_and(|s| s.scrolling);
                    in_growing_row |= self.grows.get(&parent.id).is_some_and(|g| g.moving());
                    a = step_up(pi);
                }
                let secure = n.is_secure();
                Some(ElementInfo {
                    trace_name: n.trace_name,
                    trace_namespace: n.trace_namespace,
                    loop_phase: self.loops.get(&n.id).map(|l| {
                        if self.reduced_motion && !matches!(n.looping.as_ref().map(|s| s.kind), Some(rune_motion::LoopKind::Sequence(_))) { 0.0 }
                        else { l.phase(self.scene.store().node_time(n.id)) }
                    }),
                    focusable: self.can_focus(i),
                    focused: self.focused.as_deref() == Some(&n.path),
                    focus_visible: self.keyboard_focus && self.focused.as_deref() == Some(&n.path),
                    inert: !self.allowed(i) || exiting,
                    exiting,
                    path: n.path.clone(),
                    key: n.key.clone(),
                    role: n.role,
                    label: n.label.clone(),
                    text: match &n.text {
                        Some(t) => (!secure).then(|| t.source_text().to_owned()),
                        // A text built from several (a rolling label's glyphs) reads as its label.
                        None => (!secure).then(|| n.label.clone()).flatten().filter(|_| n.role == "text"),
                    },
                    rect,
                    target: [n.target[0], n.target[1], n.target[2] - n.target[0], n.target[3] - n.target[1]],
                    opacity: flight.map_or_else(|| self.inh_scratch.get(i).map_or(p.opacity, |h| h.opacity), |f| f.target_alpha),
                    scale: if flight.is_some() { 1.0 } else { self.inh_scratch.get(i).map_or(p.transform.scale.x, |h| h.scale) },
                    blur: if flight.is_some() { 0.0 } else { self.inh_scratch.get(i).map_or(p.blur, |h| h.blur) },
                    radius: p.radius.0[0],
                    fill: self.color(n.id, P_FILL, n.vis.fill),
                    text_color: n.text.as_ref().map(|t| t.color).or(n.vis.icon.map(|(_, c)| c)).map(|c| self.color(n.id, P_TEXT, c)),
                    hover: if n.focus.highlight.is_some() && self.highlight_visible(i) { 1.0 }
                        else { self.scene.get(n.id, P_HOVER).unwrap_or(0.0) },
                    press: self.scene.get(n.id, P_PRESS).unwrap_or(0.0),
                    moving,
                    parent: n.parent,
                    visible,
                    revealing,
                    in_growing_row,
                    scrolling,
                    live_intent,
                    caret: self.caret_rect(&self.nodes, i),
                    drag_out: n.drag_out.is_some(),
                    help: n.help.as_ref().filter(|_| !n.help_cut || n.elided.is_some()).cloned(),
                    selected: n.text.as_ref().filter(|t| t.selectable && !secure).and_then(|t| {
                        let (a, b) = selectable::range_in(&sel_ranges, i)?;
                        t.copy_slice(a as u32, b as u32)
                    }),
                    level: n.a11y.level,
                    expanded: n.a11y.expanded,
                    item_selected: n.focus.selected,
                    stuck: self.stuck.get(i).copied().unwrap_or(0.0),
                    virtual_extent: self.virtuals.contains_key(&n.path) && n.vis.fill.a == 0.0
                        && n.vis.border_width == 0.0 && n.vis.shadows.is_empty() && n.text.is_none() && n.vis.icon.is_none(),
                })
            })
            .collect();
        for f in &self.flights {
            let r = if self.reduced_motion { f.source } else { f.presented };
            let c = if self.reduced_motion { f.source_clip } else { f.clip };
            let path = format!("flight-source/{}", f.target_key);
            let t = f.destination;
            elements.push(ElementInfo {
                trace_name: None,
                trace_namespace: None,
                loop_phase: None,
                focusable: false, focused: false, focus_visible: false, inert: true, exiting: false,
                key: Some(path.clone()), path, role: "text", label: None, text: None,
                rect: [r.x, r.y, r.w, r.h], target: [t.x, t.y, t.w, t.h],
                opacity: f.source_alpha, scale: 1.0, blur: 0.0, radius: 0.0, fill: Color::TRANSPARENT, text_color: None,
                hover: 0.0, press: 0.0, moving: true, parent: None,
                visible: [r.x.max(c[0]), r.y.max(c[1]), (r.edges()[2].min(c[2]) - r.x.max(c[0])).max(0.0),
                    (r.edges()[3].min(c[3]) - r.y.max(c[1])).max(0.0)],
                revealing: true, in_growing_row: false, scrolling: false, virtual_extent: false, live_intent: 0, caret: None, drag_out: false,
                help: None, selected: None, level: None, expanded: None, item_selected: false, stuck: 0.0,
            });
        }
        self.link_elements(&mut elements);
        self.highlight_elements(&mut elements);
        elements
    }
}
