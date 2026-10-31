//! Platform-neutral accessible tree: names, which controls collapse, and the relationships
//! between them. The web mirror projects this tree into the DOM. The inspector reports the
//! same computed name. A later AccessKit or UIKit adapter can read it without a second pass.

use crate::el::a11y::{CheckState, Current, LabelSource};
use crate::Orientation;

use super::*;

/// One exposed node, in tree order. A collapsed control's descendants are absent.
pub(crate) struct SemanticNode {
    pub origin: usize,
    pub kind: &'static str,
    pub path: String,
    pub parent: Option<String>,
    pub name: Option<String>,
    pub text: String,
    pub labelled_by: Option<String>,
    pub tab: Option<i32>,
    pub level: Option<u32>,
    pub expanded: Option<bool>,
    pub disabled: bool,
    pub checked: Option<CheckState>,
    pub selected: Option<bool>,
    pub active_descendant: Option<String>,
    pub controls: Option<String>,
    pub orientation: Option<Orientation>,
    pub help: Option<String>,
    pub modal: bool,
    pub invalid: bool,
    pub range: Option<crate::ValueRange>,
    pub set_size: Option<i32>,
    pub pos_in_set: Option<i32>,
    pub pressed: Option<bool>,
    pub current: Option<Current>,
    pub multiselectable: bool,
}

impl SemanticNode {
    /// The name a host announces: the explicit name, otherwise the visible text.
    pub(crate) fn accessible_name(&self) -> Option<&str> {
        self.name.as_deref().map(str::trim).filter(|s| !s.is_empty()).or_else(|| {
            let text = self.text.trim();
            (!text.is_empty()).then_some(text)
        })
    }
}

/// A heading with no authored level is level 2, in the tree and in the mirror.
pub(crate) fn exposed_level(role: &str, level: Option<u32>) -> Option<u32> {
    if role == "heading" { Some(level.unwrap_or(2).clamp(1, 6)) } else { level }
}

pub(super) fn round1(v: f32) -> f32 { (v * 10.0).round() / 10.0 }

pub(super) fn rect4(b: [f32; 4]) -> [f32; 4] {
    [round1(b[0]), round1(b[1]), round1((b[2] - b[0]).max(0.0)), round1((b[3] - b[1]).max(0.0))]
}

pub(super) fn clean_slice(spec: &crate::el::TextSpec, a: usize, b: usize) -> String {
    let Some(slice) = spec.content.get(a..b) else { return String::new() };
    if spec.uncopied.is_empty() { return slice.to_owned(); }
    let mut excluded: Vec<_> = spec.uncopied.iter().filter_map(|&(start, end)| {
        let (start, end) = (start as usize, end as usize);
        (start < end && start < b && end > a).then_some((start, end))
    }).collect();
    excluded.sort_unstable();
    let mut next = 0;
    let mut out = String::with_capacity(slice.len());
    for (offset, ch) in slice.char_indices() {
        let start = a + offset;
        let end = start + ch.len_utf8();
        while next < excluded.len() && excluded[next].1 <= start { next += 1; }
        if next == excluded.len() || excluded[next].0 >= end { out.push(ch); }
    }
    out
}

impl<S: 'static> Runtime<S> {
    /// Computed names, keyed by node index. An element the tree does not expose has none.
    pub(super) fn semantic_names(&self) -> Vec<Option<String>> {
        let mut names = vec![None; self.nodes.len()];
        for node in self.semantic_tree() {
            names[node.origin] = node.accessible_name().map(str::to_owned);
        }
        names
    }

    pub(super) fn semantic_tree(&self) -> Vec<SemanticNode> {
        let places = self.virtual_places();
        let mut out = Vec::new();
        let visible = |i: usize| self.in_scope(i) && !self.subtree_exiting(i) && !self.subtree_unmounted(i)
            && std::iter::successors(Some(i), |&j| self.nodes[j].parent).all(|j| self.nodes[j].vis.opacity > 0.01);
        let owners = self.subtree_owners(|i| self.nodes[i].input.is_some() && visible(i));
        let caption_controls = self.subtree_owners(|i| {
            let n = &self.nodes[i];
            (n.input.is_some() || n.role == "combobox") && visible(i)
        });
        let captions = self.subtree_owners(|i| {
            let n = &self.nodes[i];
            n.role == "label" && visible(i) && {
                let rect = rect4(self.drawn_box(i));
                rect[2] > 0.0 && rect[3] > 0.0
            } && n.text.as_deref().is_some_and(|t| {
                !t.redacted && if t.uncopied.is_empty() { !t.content.trim().is_empty() }
                    else { !clean_slice(t, 0, t.content.len()).trim().is_empty() }
            })
        });
        let mut nearby_caption = vec![None; self.nodes.len()];
        for (i, node) in self.nodes.iter().enumerate() {
            nearby_caption[i] = captions[i].or_else(|| node.parent.and_then(|p| nearby_caption[p]));
        }
        let caption_for = |i: usize, name: Option<&str>| {
            let caption = nearby_caption[i]?;
            let spec = self.nodes[caption].text.as_deref()?;
            let name = name?.trim();
            let matches = if spec.uncopied.is_empty() { name == spec.content.trim() }
                else { name == clean_slice(spec, 0, spec.content.len()).trim() };
            matches.then(|| self.nodes[caption].path.clone())
        };
        let mut tabbable = vec![false; self.nodes.len()];
        for i in self.tab_order() { tabbable[i] = true; }
        let mut labels: Vec<_> = self.nodes.iter().map(|n| n.label.as_deref()
            .filter(|s| !s.trim().is_empty()).map(|s| (s, n.a11y.label_source))).collect();
        for (i, n) in self.nodes.iter().enumerate().rev() {
            if n.input.is_none() && (matches!(n.role, "textbox" | "combobox") || n.a11y.label_source == LabelSource::Composed)
                && let Some(label) = labels[i] && let Some(editor) = owners[i].or(caption_controls[i])
                && labels[editor].is_none_or(|(_, source)| source < label.1)
            {
                labels[editor] = Some(label);
            }
        }
        let (control_path, control_target) = self.control_facts();
        let nlen = self.nodes.len();
        let mut shell_of = vec![None; nlen];
        let mut claimed = vec![false; nlen];
        for (i, owner) in owners.iter().enumerate() {
            if self.nodes[i].role != "combobox" { continue; }
            let Some(editor) = *owner else { continue; };
            if editor == i { continue; }
            if claimed[editor] { shell_of[editor] = None; }
            else { claimed[editor] = true; shell_of[editor] = Some(i); }
        }
        let shell_expanded: Vec<Option<bool>> = shell_of.iter().map(|shell| shell.and_then(|s| self.nodes[s].a11y.expanded)).collect();
        let mut retained: Vec<Option<String>> = vec![None; nlen];
        let mut i = 0;
        while i < self.nodes.len() {
            let n = &self.nodes[i];
            let end = self.subtree_end(i);
            if !visible(i) || (matches!(n.role, "none" | "presentation") && !control_target[i]) {
                retained[i] = n.parent.and_then(|p| retained[p].clone());
                i += 1;
                continue;
            }
            let parent = n.parent.and_then(|p| retained[p].clone());
            if owners[i].is_some_and(|editor| shell_of[editor] == Some(i)) {
                retained[i] = parent;
                i += 1;
                continue;
            }
            let tab = self.can_focus(i).then_some(if tabbable[i] { n.focus.order } else { -1 });
            let name = labels[i].map(|(s, _)| s.to_owned());
            if n.input.is_some() {
                let kind = if shell_of[i].is_some() || n.role == "combobox" { "combobox" } else { "textbox" };
                let mut node = self.semantic_node(i, kind, parent, name, tab, &places);
                if node.expanded.is_none() { node.expanded = shell_expanded[i]; }
                node.labelled_by = caption_for(i, node.name.as_deref());
                retained[i] = Some(node.path.clone());
                out.push(node);
                i = end;
                continue;
            }
            let rect = rect4(self.drawn_box(i));
            let semantic_kind = if self.active_modal() == Some(i) && !matches!(n.role, "dialog" | "alertdialog") {
                Some("dialog")
            } else { match n.role {
                "group" if labels[i].is_some() || tab.is_some() || n.focus.group => Some("group"),
                "textbox" => Some("group"),
                "text" | "heading" | "button" | "group" | "label" => None,
                role => Some(role),
            }};
            let control_owns_name = owners[i].or(caption_controls[i]).is_some_and(|editor| labels[i].is_some()
                && labels[i].map(|(label, _)| label) == labels[editor].map(|(label, _)| label))
                && (n.role == "textbox" || n.a11y.label_source == LabelSource::Composed)
                && tab.is_none() && !n.focus.group;
            let mut semantic_kind = if semantic_kind == Some("group") && control_owns_name { None } else { semantic_kind };
            if control_target[i] && semantic_kind.is_none_or(|k| matches!(k, "none" | "presentation")) {
                semantic_kind = Some("group");
            }
            let button_like = (n.role == "button" || n.handlers.on_press.is_some()) && !n.window_drag;
            // A nested press, field, slider or link is its own tab stop. Collapsing would drop it.
            let blocks = self.blocks_collapse(i + 1, end);
            if let Some(kind) = semantic_kind {
                let mut node = self.semantic_node(i, kind, parent.clone(), name, tab, &places);
                if matches!(kind, "group" | "combobox") {
                    node.labelled_by = caption_for(i, node.name.as_deref());
                }
                if !matches!(kind, "checkbox" | "radio" | "switch")
                    && let Some(spec) = n.text.as_deref().filter(|t| !t.redacted)
                {
                    node.text = clean_slice(spec, 0, spec.content.len());
                }
                let keeps_editor = kind == "combobox" && owners[i].is_some_and(|editor| editor != i);
                let collapse = !keeps_editor && !blocks && (kind == "combobox"
                    || ((button_like || matches!(kind, "checkbox" | "radio" | "switch")) && kind != "row"));
                if kind == "combobox" {
                    if collapse { node.text = self.descendant_label(i + 1, end); }
                } else if matches!(kind, "checkbox" | "radio" | "switch") && collapse {
                    let mut name_in_content = false;
                    for j in i..end {
                        let child = &self.nodes[j];
                        if !visible(j) || child.input.is_some() { continue; }
                        let Some(spec) = child.text.as_deref().filter(|t| !t.redacted) else { continue };
                        let text = clean_slice(spec, 0, spec.content.len());
                        let text = text.trim();
                        if text.is_empty() { continue; }
                        name_in_content |= node.name.as_deref().is_some_and(|name| name.trim() == text);
                        if !node.text.is_empty() { node.text.push(' '); }
                        node.text.push_str(text);
                    }
                    if name_in_content { node.name = None; }
                } else if matches!(kind, "tab" | "option" | "row" | "gridcell") {
                    let flatten = button_like && !blocks && kind != "row";
                    if flatten {
                        let (text, repeated) = self.flattened_contents(i, end, node.name.as_deref(), visible);
                        if !text.is_empty() { node.text = text; }
                        if repeated { node.name = None; }
                    } else if matches!(kind, "tab" | "option") {
                        if node.name.as_deref().is_some_and(|name| {
                            (i..end).any(|j| {
                                let child = &self.nodes[j];
                                visible(j) && child.input.is_none() && child.text.as_deref()
                                    .filter(|t| !t.redacted)
                                    .is_some_and(|t| clean_slice(t, 0, t.content.len()).trim() == name.trim())
                            })
                        }) {
                            node.name = None;
                        } else if node.name.is_none() {
                            let label = self.descendant_label(i + 1, end);
                            if !label.is_empty() { node.name = Some(label); }
                        }
                    }
                } else if matches!(kind, "menuitem" | "menuitemcheckbox" | "menuitemradio") && node.name.is_none() {
                    let label = self.descendant_label(i + 1, end);
                    if !label.is_empty() { node.name = Some(label); }
                }
                retained[i] = Some(node.path.clone());
                out.push(node);
                if collapse {
                    i = end;
                    continue;
                }
                i += 1;
                continue;
            } else if button_like && rect[2] > 0.0 && rect[3] > 0.0 {
                let explicit = n.label.clone().filter(|s| !s.trim().is_empty());
                let label = explicit.clone().unwrap_or_else(|| {
                    if blocks { String::new() } else { self.descendant_label(i, end) }
                });
                if !label.is_empty() || blocks {
                    let mut node = self.semantic_node(i, "button", parent.clone(), None, tab, &places);
                    if blocks {
                        node.name = explicit.or_else(|| (!label.is_empty()).then_some(label));
                    } else {
                        node.text = label;
                    }
                    retained[i] = Some(node.path.clone());
                    out.push(node);
                    if !blocks {
                        i = end;
                        continue;
                    }
                    i += 1;
                    continue;
                }
            }
            if let Some(spec) = n.text.as_deref().filter(|t| !t.redacted && rect[2] > 0.0 && rect[3] > 0.0) {
                let text = clean_slice(spec, 0, spec.content.len());
                let linked = spec.links.iter().enumerate().any(|(k, _)| self.accepts_input(i) && self.link_eligible(i, k));
                if !text.trim().is_empty() || linked {
                    let kind = if n.role == "heading" { "heading" } else if n.role == "label" { "label" } else { "text" };
                    let mut node = self.semantic_node(i, kind, parent.clone(), name, tab, &places);
                    node.text = text;
                    retained[i] = Some(node.path.clone());
                    out.push(node);
                }
            } else if n.role == "heading" && let Some(lbl) = n.label.clone().filter(|s| !s.trim().is_empty()) {
                let mut node = self.semantic_node(i, "heading", parent, name, tab, &places);
                node.text = lbl;
                retained[i] = Some(node.path.clone());
                out.push(node);
            }
            if retained[i].is_none() { retained[i] = n.parent.and_then(|p| retained[p].clone()); }
            i += 1;
        }
        for node in &mut out {
            node.controls = control_path.get(node.origin).and_then(|p| p.clone());
            if node.controls.is_none()
                && let Some(shell) = shell_of.get(node.origin).copied().flatten()
            {
                node.controls = control_path.get(shell).and_then(|p| p.clone());
            }
        }
        self.assign_active_descendants(&mut out);
        out
    }

    fn subtree_end(&self, root: usize) -> usize {
        let mut j = root + 1;
        while j < self.nodes.len() {
            if std::iter::successors(self.nodes[j].parent, |&p| self.nodes[p].parent).any(|p| p == root) {
                j += 1;
            } else {
                break;
            }
        }
        j
    }

    /// A descendant that owns a tab stop. The parent stays in the tree and so does this node.
    fn blocks_collapse(&self, start: usize, end: usize) -> bool {
        (start..end).any(|j| {
            let n = &self.nodes[j];
            n.handlers.on_press.is_some()
                || n.role == "button"
                || n.input.is_some()
                || n.focus.enabled
                || n.focus.step.is_some()
                || self.has_focusable_link(j)
        })
    }

    fn descendant_label(&self, root: usize, end: usize) -> String {
        let mut parts = Vec::new();
        for j in root..end {
            if !self.in_scope(j) || self.subtree_exiting(j) || self.nodes[j].vis.opacity <= 0.01 { continue; }
            if let Some(lbl) = self.nodes[j].label.as_deref().filter(|s| !s.trim().is_empty()) {
                parts.push(lbl.trim().to_owned());
                continue;
            }
            if let Some(spec) = self.nodes[j].text.as_deref().filter(|t| !t.redacted && self.nodes[j].input.is_none()) {
                let s = clean_slice(spec, 0, spec.content.len());
                if !s.trim().is_empty() { parts.push(s.trim().to_owned()); }
            }
        }
        parts.join(" ")
    }

    /// Visible text of a flattened option, tab, or row.
    ///
    /// A caption label and an editor value stay with their owners. `repeated` means the explicit
    /// name only repeats that text, so the control should not say it twice.
    fn flattened_contents(&self, root: usize, end: usize, name: Option<&str>, shown: impl Fn(usize) -> bool) -> (String, bool) {
        let name = name.map(str::trim).filter(|s| !s.is_empty());
        let mut text = String::new();
        let mut repeated = false;
        for j in root..end {
            if !shown(j) || self.nodes[j].input.is_some() || self.nodes[j].role == "label" { continue; }
            let Some(spec) = self.nodes[j].text.as_deref().filter(|t| !t.redacted) else { continue; };
            let piece = clean_slice(spec, 0, spec.content.len());
            let piece = piece.trim();
            if piece.is_empty() { continue; }
            repeated |= name.is_some_and(|name| name == piece);
            if !text.is_empty() { text.push(' '); }
            text.push_str(piece);
        }
        repeated |= name.is_some_and(|name| name == text.as_str());
        (text, repeated)
    }

    /// 1-based place and full count for a virtual row. The mounted window is not the set.
    fn virtual_places(&self) -> Vec<Option<(i32, i32)>> {
        let mut out = vec![None; self.nodes.len()];
        if self.virtuals.is_empty() { return out; }
        let mut by_list: HashMap<&str, HashMap<&str, usize>> = HashMap::default();
        for (path, list) in &self.virtuals {
            let mut keys = HashMap::with_capacity_and_hasher(list.keys.len(), Default::default());
            for (idx, key) in list.keys.iter().enumerate() {
                keys.insert(key.as_str(), idx);
            }
            by_list.insert(path.as_str(), keys);
        }
        for (i, node) in self.nodes.iter().enumerate() {
            let Some(key) = node.key.as_deref() else { continue };
            let Some(parent) = node.parent else { continue };
            let Some(keys) = by_list.get(self.nodes[parent].path.as_str()) else { continue };
            let Some(&idx) = keys.get(key) else { continue };
            let set = i32::try_from(keys.len()).unwrap_or(i32::MAX);
            let pos = i32::try_from(idx + 1).unwrap_or(i32::MAX);
            out[i] = Some((pos, set));
        }
        out
    }

    fn semantic_node(&self, i: usize, kind: &'static str, parent: Option<String>, name: Option<String>, tab: Option<i32>, places: &[Option<(i32, i32)>]) -> SemanticNode {
        let n = &self.nodes[i];
        let (pos_in_set, set_size) = places.get(i).copied().flatten().map(|(pos, set)| (Some(pos), Some(set))).unwrap_or((None, None));
        // Options, tabs and tree items are a selection. Rows and cells are one only when the
        // author marked them, so a header row is not "not selected".
        let selected = (matches!(kind, "option" | "tab" | "treeitem") || n.focus.in_selection).then_some(n.focus.selected);
        SemanticNode {
            origin: i,
            kind,
            path: n.path.clone(),
            parent,
            name,
            text: String::new(),
            labelled_by: None,
            tab,
            level: match kind {
                "heading" => exposed_level("heading", n.a11y.level),
                "treeitem" => n.a11y.level,
                _ => None,
            },
            expanded: if matches!(kind, "text" | "heading" | "label") { None } else { n.a11y.expanded },
            disabled: n.disabled,
            checked: n.a11y.checked,
            selected,
            active_descendant: None,
            controls: None,
            orientation: n.a11y.orientation,
            help: n.help.clone().filter(|_| !n.help_cut || n.elided.is_some()),
            modal: self.active_modal() == Some(i),
            invalid: n.a11y.invalid,
            range: n.a11y.range,
            set_size,
            pos_in_set,
            pressed: n.a11y.pressed,
            current: n.a11y.current,
            multiselectable: n.a11y.multiselectable,
        }
    }

    /// Point a keyboard owner at its cursor-marked mounted descendant.
    ///
    /// Selection is not the cursor. A nearer focusable grid or tree keeps its own row.
    /// An editor points at a cursor only when that cursor is owned by the element `controls` names.
    /// Two cursors under one owner, or a stale control, leave the relationship unset.
    fn assign_active_descendants(&self, items: &mut [SemanticNode]) {
        let n = self.nodes.len();
        let mut keyboard_owner = vec![None; n];
        for i in 0..n {
            let parent = self.nodes[i].parent;
            let role = self.nodes[i].role;
            keyboard_owner[i] = if matches!(role, "tree" | "treegrid" | "grid") && self.can_focus(i) {
                Some(i)
            } else {
                parent.and_then(|p| keyboard_owner[p])
            };
        }
        #[derive(Clone, Copy)]
        enum Slot { Empty, One(usize), Many }
        let mut slot = vec![Slot::Empty; n];
        for (k, item) in items.iter().enumerate() {
            if !self.nodes[item.origin].focus.cursor { continue; }
            let Some(owner) = keyboard_owner[item.origin].filter(|&owner| owner != item.origin) else { continue; };
            slot[owner] = match slot[owner] {
                Slot::Empty => Slot::One(k),
                Slot::One(_) | Slot::Many => Slot::Many,
            };
        }
        for i in 0..items.len() {
            if let Slot::One(desc) = slot[items[i].origin] {
                let path = items[desc].path.clone();
                items[i].active_descendant = Some(path);
            }
        }
        let Some(focus) = self.focused_index().filter(|&i| self.nodes[i].input.is_some()) else { return };
        let Some(editor) = items.iter().position(|item| item.origin == focus) else { return; };
        let Some(controlled) = items[editor].controls.clone() else { return };
        let by_path: HashMap<&str, usize> = items.iter().enumerate().map(|(k, it)| (it.path.as_str(), k)).collect();
        if !by_path.contains_key(controlled.as_str()) { return; }
        let mut cursor = None;
        for (k, item) in items.iter().enumerate() {
            if !self.nodes[item.origin].focus.cursor || item.path == controlled { continue; }
            let mut parent = item.parent.as_deref();
            let mut owned = false;
            while let Some(path) = parent {
                if path == controlled { owned = true; break; }
                parent = by_path.get(path).and_then(|&j| items[j].parent.as_deref());
            }
            if !owned { continue; }
            cursor = match cursor {
                None => Some(k),
                Some(_) => return,
            };
        }
        if let Some(k) = cursor {
            items[editor].active_descendant = Some(items[k].path.clone());
        }
    }
}
