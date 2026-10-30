//! Browser semantics project retained nodes and the runtime's existing link observations.
use super::super::*;
use serde::{Serialize, Serializer};

#[derive(Clone, Copy, PartialEq, Eq)]
struct Id(u64, Option<usize>);

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)?;
        if let Some(link) = self.1 { write!(f, "#link-{link}")?; }
        Ok(())
    }
}

impl Serialize for Id {
    fn serialize<T: Serializer>(&self, serializer: T) -> Result<T::Ok, T::Error> {
        serializer.collect_str(self)
    }
}

#[derive(Serialize)]
struct Edit<'a> {
    text: Option<&'a str>,
    base: usize,
    anchor: u32,
    focus: u32,
    secure: bool,
    readonly: bool,
    single: bool,
    placeholder: &'a str,
}

#[derive(Serialize)]
struct Semantic<'a> {
    id: Id,
    parent: Option<Id>,
    key: Option<&'a str>,
    role: &'static str,
    label: Option<&'a str>,
    text: Option<&'a str>,
    after: Option<&'a str>,
    help: Option<&'a str>,
    tab: i32,
    focusable: bool,
    press: bool,
    disabled: bool,
    expanded: Option<bool>,
    checked: Option<&'static str>,
    selected: bool,
    level: Option<u32>,
    modal: bool,
    edit: Option<Edit<'a>>,
}

#[derive(Serialize)]
struct Packet<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    nodes: Option<Vec<Semantic<'a>>>,
    bounds: &'a [(Id, [f32; 4])],
    focus: Option<Id>,
    modal: bool,
}

/// Delivery state belongs to one browser shell. Its buffers contain mounted observations,
/// not application history. The focused text context remains owned by `web_context`.
#[derive(Default)]
pub(crate) struct Mirror {
    commit: Option<u64>,
    focus: Option<Id>,
    rects: Vec<[f32; 4]>,
    live: HashMap<NodeId, u128>,
    bounds: Vec<(Id, [f32; 4])>,
    bytes: Vec<u8>,
}

impl Mirror {
    pub(crate) fn delivered(&mut self) {
        if self.bytes.capacity() > 1024 * 1024 { self.bytes = Vec::new(); }
    }
    pub(crate) fn sync<S: 'static>(&mut self, rt: &Runtime<S>, context_changed: bool) -> Option<&str> {
        let focus = rt.focused_index().map(|i| Id(rt.nodes[i].id.0, rt.focused_link_index(i)));
        let mut changed = self.commit != Some(rt.commits) || self.focus != focus || self.rects.len() != rt.nodes.len();
        self.live.retain(|id, _| rt.node_index.contains_key(id));
        for node in &rt.nodes {
            if let Some(revision) = node.live_text_seen {
                changed |= self.live.insert(node.id, revision) != Some(revision);
            } else { self.live.remove(&node.id); }
        }
        self.bounds.clear();
        if self.rects.len() > rt.nodes.len() {
            self.rects.truncate(rt.nodes.len());
            self.rects.shrink_to_fit();
            self.bounds.shrink_to(rt.nodes.len());
            self.live.shrink_to_fit();
        }
        self.rects.resize(rt.nodes.len(), [f32::NAN; 4]);
        for (i, node) in rt.nodes.iter().enumerate() {
            let rect = rt.element_geometry(i).map_or([0.0; 4], |g| g.visible);
            if changed || self.rects[i] != rect {
                self.bounds.push((Id(node.id.0, None), rect));
                rt.visit_link_elements(i, rect, |link| self.bounds.push((Id(node.id.0, Some(link.index)), link.visible)));
                self.rects[i] = rect;
            }
        }
        if !changed && !context_changed && self.bounds.is_empty() { return None; }
        let packet = Packet {
            nodes: changed.then(|| rt.web_semantics()), bounds: &self.bounds, focus,
            modal: rt.active_modal().is_some(),
        };
        self.bytes.clear();
        serde_json::to_writer(&mut self.bytes, &packet).expect("browser semantics contain serializable values");
        self.commit = Some(rt.commits);
        self.focus = focus;
        Some(std::str::from_utf8(&self.bytes).expect("JSON serialization emits UTF-8"))
    }
}

impl<S: 'static> Runtime<S> {
    fn web_semantics(&self) -> Vec<Semantic<'_>> {
        let mut hidden = vec![false; self.nodes.len()];
        let mut keep = vec![false; self.nodes.len()];
        for (i, n) in self.nodes.iter().enumerate() {
            hidden[i] = n.parent.is_some_and(|p| hidden[p]) || n.inert || n.vis.opacity <= 0.0
                || !self.scene.contains(n.id) || self.is_exiting(n.id);
            keep[i] = !hidden[i] && self.in_scope(i);
        }
        for i in (0..self.nodes.len()).rev() {
            if keep[i] && let Some(parent) = self.nodes[i].parent { keep[parent] = true; }
        }
        let mut labels: Vec<_> = self.nodes.iter().map(|n| n.label.as_deref().filter(|label| !label.is_empty())).collect();
        let owners = self.input_owners(|j| !hidden[j] && self.in_scope(j));
        for (i, n) in self.nodes.iter().enumerate() {
            if n.input.is_none() && n.role == "textbox" && let Some(label) = labels[i]
                && let Some(editor) = owners[i]
            {
                labels[editor] = labels[editor].or(Some(label));
            }
        }
        let tab: HashSet<_> = self.tab_order().into_iter().collect();
        let mut nodes = Vec::with_capacity(self.nodes.len());
        for (i, n) in self.nodes.iter().enumerate().filter(|(i, _)| keep[*i]) {
            let secure = n.is_secure();
            let edit = n.input.as_ref().map(|input| {
                let focused = input.access.is_some() && self.focused.as_deref() == Some(&n.path);
                let (base, text, anchor, focus) = if secure { (0, "", 0, 0) }
                    else if input.access.is_none() { (0, input.edit.text.as_str(), 0, 0) }
                    else if focused { (0, "", 0, 0) }
                    else { super::context_window(&input.edit) };
                Edit { text: (!focused).then_some(text), base, anchor, focus, secure, readonly: input.access.is_none(),
                    single: input.single_line, placeholder: &input.placeholder }
            });
            let links = n.text.as_ref().is_some_and(|t| !t.links.is_empty());
            let shell = n.role == "textbox" && n.input.is_none();
            let mut inline = Vec::new();
            if links {
                let visible = self.element_geometry(i).map_or([0.0; 4], |g| g.visible);
                self.visit_link_elements(i, visible, |link| {
                    if link.text.is_some() { inline.push(link); }
                });
                inline.sort_by_key(|link| n.text.as_ref().unwrap().links[link.index].start);
            }
            nodes.push(Semantic {
                id: Id(n.id.0, None), parent: n.parent.map(|p| Id(self.nodes[p].id.0, None)),
                key: n.input.as_ref().map(|_| n.path.as_str()), role: if shell { "group" } else { n.role }, label: labels[i],
                text: (!secure && n.input.is_none() && self.in_scope(i)).then(|| n.text.as_ref().and_then(|t| {
                    inline.first().map_or(Some(t.source_text()), |link| t.source_slice(0, t.links[link.index].start))
                })).flatten(),
                after: None,
                help: n.help.as_deref().filter(|_| !n.help_cut || n.elided.is_some()),
                tab: if tab.contains(&i) && !links { n.focus.order.max(0) } else { -1 },
                focusable: !links && self.can_focus(i), press: self.allowed(i) && n.on_press.is_some(), disabled: n.a11y.disabled,
                expanded: n.a11y.expanded,
                checked: n.a11y.checked.map(|state| if state.indeterminate { "mixed" } else if state.checked { "true" } else { "false" }),
                selected: n.focus.selected, level: n.a11y.level, modal: self.active_modal() == Some(i), edit,
            });
            if let Some(spec) = n.text.as_ref() {
                let mut end = 0;
                for (at, link) in inline.iter().enumerate() {
                    let range = &spec.links[link.index];
                    let start = end.max(range.start);
                    end = end.max(range.end);
                    let next = inline.get(at + 1).map_or(spec.content.len() as u32, |link| spec.links[link.index].start).max(end);
                    nodes.push(Semantic {
                        id: Id(n.id.0, Some(link.index)), parent: Some(Id(n.id.0, None)), key: None,
                        role: link.role, label: link.label, text: spec.source_slice(start, end), after: spec.source_slice(end, next), help: None,
                        tab: if tab.contains(&i) { n.focus.order.max(0) } else { -1 },
                        focusable: self.can_focus(i), press: self.allowed(i) && spec.on_link.is_some(),
                        disabled: n.a11y.disabled, expanded: None, checked: None, selected: false, level: None, modal: false, edit: None,
                    });
                }
            }
        }
        nodes
    }

    /// DOM presence does not grant authority. Revalidate the current retained target.
    pub(crate) fn web_accessibility_action(&mut self, event: &serde_json::Value) -> bool {
        if event["action"] == "blur" {
            if self.set_focus(None, true) { self.dirty = true; }
            return true;
        }
        let Some(raw) = event["id"].as_str() else { return false };
        let (raw, link) = match raw.split_once("#link-") {
            Some((node, link)) => {
                let Ok(link) = link.parse::<usize>() else { return false };
                (node, Some(link))
            }
            None => (raw, None),
        };
        let Some(id) = raw.parse::<u64>().ok().map(NodeId) else { return false };
        let Some(&i) = self.node_index.get(&id) else { return false };
        if !self.allowed(i) || self.subtree_exiting(i) || self.subtree_unmounted(i) { return false; }
        if std::iter::successors(Some(i), |&j| self.nodes[j].parent).any(|j| self.nodes[j].vis.opacity <= 0.0) { return false; }
        let path = self.nodes[i].path.clone();
        if let Some(k) = link {
            if self.nodes[i].text.as_ref().is_none_or(|t| k >= t.links.len()) { return false; }
            return match event["action"].as_str() {
                Some("focus") => self.focus_link(i, k),
                Some("activate") if self.nodes[i].text.as_ref().is_some_and(|t| t.on_link.is_some()) => {
                    self.queue.push(Action::Link(path, k)); true
                }
                _ => false,
            };
        }
        match event["action"].as_str() {
            Some("focus") if self.can_focus(i) => {
                if self.set_focus(Some(path.clone()), true) { self.dirty = true; }
                self.pending.scroll_to.push((None, path));
                true
            }
            Some("activate") if self.nodes[i].on_press.is_some() => {
                self.queue.push(Action::Press(path, crate::PointerButton::Primary)); true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
#[path = "accessibility/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "accessibility/focus_tests.rs"]
mod focus_tests;
