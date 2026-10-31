//! Hidden semantic DOM mirror for the browser shell: headings, text runs with inline links,
//! buttons and native editors positioned over the canvas so assistive tech and browser
//! Find-in-page see the scene.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
use std::hash::{Hash, Hasher};

use serde_json::{Value, json};

use super::*;
use super::semantic::{clean_slice, round1};
pub(super) use super::semantic::rect4;
use crate::el::a11y::{CheckState, Current};

mod action;
mod controls;
mod editor;

pub(super) use editor::context_window;

#[derive(Default)]
pub(crate) struct A11yMirror {
    signature: Option<u64>,
    sources: Vec<A11ySource>,
    focus: Option<(String, Option<usize>)>,
    modal: bool,
    json: Vec<u8>,
}

struct A11ySource {
    path: String,
    primary: NodeId,
    extra: Vec<NodeId>,
    rect: [f32; 4],
}

impl A11yMirror {
    pub(crate) fn delivered(&mut self) {
        if self.json.capacity() > 1024 * 1024 { self.json = Vec::new(); }
    }

    /// The DOM kept an open composition and did not apply the last tree. The next delivery
    /// sends the items again.
    pub(crate) fn release_hold(&mut self) {
        self.signature = None;
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct A11yItem {
    pub kind: &'static str,
    pub path: String,
    /// `[x, y, w, h]` in viewport points (CSS pixels).
    pub rect: [f32; 4],
    pub font_size: f32,
    pub level: Option<u32>,
    pub expanded: Option<bool>,
    pub text: String,
    pub runs: Vec<A11yRun>,
    pub name: Option<String>,
    pub labelled_by: Option<String>,
    pub parent: Option<String>,
    pub tab: Option<i32>,
    pub disabled: bool,
    pub checked: Option<CheckState>,
    /// `None` when this item is not in a selection. A header row stays unset.
    pub selected: Option<bool>,
    /// Path of the mounted cursor descendant this owner points at.
    pub active_descendant: Option<String>,
    /// Resolved path of the node this item controls. Absent when unset, stale, or ambiguous.
    pub controls: Option<String>,
    pub orientation: Option<crate::Orientation>,
    /// This node owns `on_press` and passes the current admission. Not a tab stop.
    pub pressable: bool,
    pub help: Option<String>,
    pub modal: bool,
    pub invalid: bool,
    pub range: Option<crate::ValueRange>,
    /// Full count of a virtual list, tree or table, not the mounted window.
    pub set_size: Option<i32>,
    /// 1-based place in that set.
    pub pos_in_set: Option<i32>,
    /// `None` when this is not a toggle.
    pub pressed: Option<bool>,
    pub current: Option<Current>,
    pub multiselectable: bool,
    pub sort: Option<crate::SortDirection>,
    pub value_text: Option<String>,
    pub key_shortcuts: Option<String>,
    pub edit: Option<A11yEdit>,
}

#[derive(Debug, PartialEq)]
pub(crate) struct A11yEdit {
    text: Option<String>,
    base: usize,
    anchor: u32,
    focus: u32,
    secure: bool,
    readonly: bool,
    single: bool,
    placeholder: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct A11yRun {
    pub text: String,
    pub href: Option<String>,
    pub path: Option<String>,
    pub link: Option<usize>,
    pub role: Option<&'static str>,
    pub label: Option<String>,
    pub tab: Option<i32>,
}

impl A11yRun {
    fn plain(text: String) -> Self {
        Self { text, href: None, path: None, link: None, role: None, label: None, tab: None }
    }
}

impl A11yItem {
    fn record_source(sources: &mut Vec<A11ySource>, track: bool, item: &Self, primary: NodeId) {
        if track {
            sources.push(A11ySource { path: item.path.clone(), primary, extra: Vec::new(), rect: item.rect });
        }
    }

    fn to_json(&self) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert("kind".into(), json!(self.kind));
        obj.insert("path".into(), json!(self.path));
        obj.insert("rect".into(), json!(self.rect));
        if self.font_size != 0.0 { obj.insert("font_size".into(), json!(self.font_size)); }
        if let Some(l) = self.level { obj.insert("level".into(), json!(l)); }
        if let Some(x) = self.expanded { obj.insert("expanded".into(), json!(x)); }
        if !self.text.is_empty() { obj.insert("text".into(), json!(self.text)); }
        if !self.runs.is_empty() {
            let runs: Vec<Value> = self.runs.iter().map(|r| {
                let mut ro = serde_json::Map::new();
                ro.insert("text".into(), json!(r.text));
                if let Some(h) = &r.href { ro.insert("href".into(), json!(h)); }
                if let Some(p) = &r.path { ro.insert("path".into(), json!(p)); }
                if let Some(i) = r.link { ro.insert("index".into(), json!(i)); }
                if let Some(role) = r.role { ro.insert("role".into(), json!(role)); }
                if let Some(label) = &r.label { ro.insert("label".into(), json!(label)); }
                if let Some(tab) = r.tab { ro.insert("tab".into(), json!(tab)); }
                Value::Object(ro)
            }).collect();
            obj.insert("runs".into(), Value::Array(runs));
        }
        if let Some(name) = &self.name { obj.insert("name".into(), json!(name)); }
        if let Some(path) = &self.labelled_by { obj.insert("labelled_by".into(), json!(path)); }
        if let Some(parent) = &self.parent { obj.insert("parent".into(), json!(parent)); }
        if let Some(tab) = self.tab { obj.insert("tab".into(), json!(tab)); }
        if self.disabled { obj.insert("disabled".into(), json!(true)); }
        if let Some(checked) = self.checked {
            obj.insert("checked".into(), json!({"checked": checked.checked, "indeterminate": checked.indeterminate}));
        }
        if let Some(selected) = self.selected { obj.insert("selected".into(), json!(selected)); }
        if let Some(path) = &self.active_descendant { obj.insert("activedescendant".into(), json!(path)); }
        if let Some(path) = &self.controls { obj.insert("controls".into(), json!(path)); }
        if let Some(axis) = self.orientation {
            obj.insert("orientation".into(), json!(match axis {
                crate::Orientation::Horizontal => "horizontal",
                crate::Orientation::Vertical => "vertical",
            }));
        }
        if self.pressable { obj.insert("pressable".into(), json!(true)); }
        if self.invalid { obj.insert("invalid".into(), json!(true)); }
        if let Some(range) = self.range {
            obj.insert("range".into(), json!({"value": range.value, "min": range.min, "max": range.max}));
        }
        if let Some(n) = self.set_size { obj.insert("set_size".into(), json!(n)); }
        if let Some(n) = self.pos_in_set { obj.insert("pos_in_set".into(), json!(n)); }
        if let Some(pressed) = self.pressed { obj.insert("pressed".into(), json!(pressed)); }
        if let Some(current) = self.current { obj.insert("current".into(), json!(current.as_str())); }
        if self.multiselectable { obj.insert("multiselectable".into(), json!(true)); }
        if let Some(sort) = self.sort {
            obj.insert("sort".into(), json!(match sort {
                crate::SortDirection::Ascending => "ascending",
                crate::SortDirection::Descending => "descending",
            }));
        }
        if let Some(text) = &self.value_text { obj.insert("value_text".into(), json!(text)); }
        if let Some(keys) = &self.key_shortcuts { obj.insert("keyshortcuts".into(), json!(keys)); }
        if let Some(help) = &self.help { obj.insert("help".into(), json!(help)); }
        if self.modal { obj.insert("modal".into(), json!(true)); }
        if let Some(edit) = &self.edit {
            obj.insert("edit".into(), json!({
                "text": edit.text, "base": edit.base, "anchor": edit.anchor, "focus": edit.focus,
                "secure": edit.secure, "readonly": edit.readonly, "single": edit.single,
                "placeholder": edit.placeholder,
            }));
        }
        Value::Object(obj)
    }
}

fn text_runs(path: &str, spec: &crate::el::TextSpec, tab: Option<i32>, eligible: impl Fn(usize) -> bool) -> Vec<A11yRun> {
    let mut links: Vec<_> = spec.links.iter().enumerate().collect();
    links.sort_unstable_by_key(|(index, link)| super::links::link_source_order(link, *index));
    let mut out = Vec::new();
    let mut at = 0usize;
    for (idx, link) in links {
        let (la, lb) = (link.start as usize, link.end as usize);
        if la >= lb || lb > spec.content.len() || !spec.content.is_char_boundary(la)
            || !spec.content.is_char_boundary(lb) { continue; }
        if la > at {
            let before = clean_slice(spec, at, la);
            if !before.is_empty() { out.push(A11yRun::plain(before)); }
        }
        let overlap_name = (la < at).then(|| clean_slice(spec, la, lb)).filter(|s| !s.is_empty());
        let label = clean_slice(spec, la.max(at).min(lb), lb);
        if !label.is_empty() || overlap_name.is_some() {
            if eligible(idx) {
                out.push(A11yRun {
                    text: label, href: Some(link.id.clone()), path: Some(path.to_owned()), link: Some(idx),
                    role: Some(link.role), label: link.label.clone().or(overlap_name), tab,
                });
            } else if !label.is_empty() {
                out.push(A11yRun::plain(label));
            }
        }
        at = at.max(lb);
    }
    if at < spec.content.len() {
        let tail = clean_slice(spec, at, spec.content.len());
        if !tail.is_empty() { out.push(A11yRun::plain(tail)); }
    }
    out
}

impl<S: 'static> Runtime<S> {

    /// `on_press` plus the admission `web_a11y_action` rechecks. Not focus.
    pub(super) fn press_capable(&self, i: usize) -> bool {
        let n = &self.nodes[i];
        if n.handlers.on_press.is_none() || !self.accepts_input(i) || self.subtree_exiting(i) || self.subtree_unmounted(i) {
            return false;
        }
        if std::iter::successors(Some(i), |&j| self.nodes[j].parent).any(|j| self.nodes[j].vis.opacity <= 0.01) {
            return false;
        }
        let rect = rect4(self.drawn_box(i));
        rect[2] > 0.0 && rect[3] > 0.0
    }

    /// The nearest ancestor's help, skipping truncation-only tips.
    fn inherited_help(&self, mut parent: Option<usize>) -> Option<String> {
        while let Some(j) = parent {
            let node = &self.nodes[j];
            if !node.help_cut && let Some(help) = node.help.clone() {
                return Some(help);
            }
            parent = node.parent;
        }
        None
    }

    fn inherited_invalid(&self, mut parent: Option<usize>) -> bool {
        while let Some(j) = parent {
            if self.nodes[j].a11y.invalid { return true; }
            parent = self.nodes[j].parent;
        }
        false
    }

    fn project_item(&self, node: &super::semantic::SemanticNode) -> A11yItem {
        let i = node.origin;
        let source = &self.nodes[i].a11y;
        A11yItem {
            kind: node.kind, path: node.path.clone(), rect: rect4(self.drawn_box(i)), font_size: 0.0,
            level: node.level, expanded: node.expanded, text: node.text.clone(), runs: Vec::new(),
            name: node.name.clone(), labelled_by: node.labelled_by.clone(), parent: node.parent.clone(), tab: node.tab,
            disabled: node.disabled, checked: node.checked, selected: node.selected,
            active_descendant: node.active_descendant.clone(), controls: node.controls.clone(),
            orientation: node.orientation, pressable: self.press_capable(i), help: node.help.clone(),
            modal: node.modal, invalid: node.invalid, range: node.range,
            set_size: node.set_size, pos_in_set: node.pos_in_set, pressed: node.pressed,
            current: node.current, multiselectable: node.multiselectable,
            sort: source.sort, value_text: source.value_text.clone(), key_shortcuts: source.key_shortcuts.clone(),
            edit: None,
        }
    }

    /// The semantic tree as a JSON array, one object per exposed node, in tree order.
    ///
    /// The web mirror projects this same tree. Names, collapsed controls and relationships
    /// (`labelled_by`, `activedescendant`, `controls`) are already resolved.
    ///
    /// Every field name is a word:
    ///
    /// * `kind`, `path`, `rect` (`[x, y, width, height]` in points), `parent`
    /// * `name` when the accessible name is not the visible text; `text` and `runs` for that text.
    ///   A run is `{text, href, path, index, role, label, tab}`.
    /// * `level` on a heading or tree item. A heading with no authored level is 2, the same value
    ///   [`ElementInfo::level`](crate::ElementInfo::level) reports.
    /// * `expanded`, `disabled`, `invalid`, `selected`, `pressed`, `current`
    /// * `checked` as `{checked, indeterminate}` when the control is checkable
    /// * `labelled_by`, `activedescendant` and `controls` as paths of other objects in this array
    /// * `set_size` and `pos_in_set` on a virtual row: the full count, and the 1-based place in it
    /// * `multiselectable` on a grid, list or tree that selects more than one item
    /// * `tab`, `orientation`, `help`, `modal`, `range` (`{value, min, max}`), `font_size`, `pressable`
    /// * `sort` (`ascending` or `descending`), `value_text`, `keyshortcuts`
    /// * `edit` for a native editor. A focused field omits its text; the shell keeps the composition.
    ///
    /// `selected` and `pressed` are present only when that state was set, including `false`.
    /// A header row is not reported as not selected. `current` is `true`, `page`, `step`,
    /// `location`, `date` or `time`.
    pub fn a11y_json(&self) -> String {
        let values: Vec<Value> = self.web_a11y_items().iter().map(A11yItem::to_json).collect();
        serde_json::to_string(&values).unwrap_or_else(|_| "[]".into())
    }

    pub(crate) fn web_a11y_items(&self) -> Vec<A11yItem> {
        self.collect_a11y(false).0
    }

    fn collect_a11y(&self, track: bool) -> (Vec<A11yItem>, Vec<A11ySource>) {
        let mut out = Vec::new();
        let mut sources = Vec::new();
        for node in self.semantic_tree() {
            let i = node.origin;
            let n = &self.nodes[i];
            let mut item = self.project_item(&node);
            if let Some(input) = n.input.as_ref() {
                let secure = n.a11y.secure || input.edit.is_secure();
                let readonly = input.access.is_none();
                let focused = !readonly && self.focused.as_deref() == Some(n.path.as_str());
                let (base, text, anchor, focus) = if secure || focused { (0, "", 0, 0) }
                    else if readonly { (0, input.edit.text.as_str(), 0, 0) }
                    else { context_window(&input.edit) };
                // A field sets help and invalid on the textbox shell. The mirror emits the editor,
                // so those descriptions move onto it. Truncation tooltips (`help_cut`) stay put.
                if item.help.is_none() { item.help = self.inherited_help(n.parent); }
                if !item.invalid && self.inherited_invalid(n.parent) { item.invalid = true; }
                item.edit = Some(A11yEdit { text: (!focused).then(|| text.to_owned()), base, anchor, focus,
                    secure, readonly, single: input.single_line, placeholder: input.placeholder.clone() });
            } else if matches!(node.kind, "text" | "heading" | "label") {
                if let Some(spec) = n.text.as_deref().filter(|t| !t.redacted) {
                    let tab = item.tab;
                    let runs = text_runs(&n.path, spec, tab, |k| self.accepts_input(i) && self.link_eligible(i, k));
                    item.font_size = round1(spec.style.size);
                    let has_links = runs.iter().any(|run| run.href.is_some());
                    let (plain, rich) = if runs.len() == 1 && runs[0].href.is_none() {
                        (runs.into_iter().next().unwrap().text, Vec::new())
                    } else {
                        (String::new(), runs)
                    };
                    item.text = plain;
                    item.runs = rich;
                    if has_links { item.tab = item.tab.map(|_| -1); }
                } else if node.kind == "heading" {
                    item.font_size = 16.0;
                }
            }
            A11yItem::record_source(&mut sources, track, &item, n.id);
            out.push(item);
        }
        (out, sources)
    }

    /// Deliver semantic changes independently from geometry and focus changes.
    pub(crate) fn web_a11y<'a>(&self, mirror: &'a mut A11yMirror) -> Option<&'a str> {
        let focus_index = self.focused_index().filter(|&i| self.in_scope(i));
        let focus = focus_index.map(|i| (self.nodes[i].path.as_str(), self.focused_link_index(i)));
        let modal = self.active_modal().is_some();
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        focus.hash(&mut hash);
        modal.hash(&mut hash);
        self.active_modal().map(|i| self.nodes[i].path.as_str()).hash(&mut hash);
        for (i, node) in self.nodes.iter().enumerate() {
            node.id.hash(&mut hash);
            node.parent.hash(&mut hash);
            node.role.hash(&mut hash);
            node.key.hash(&mut hash);
            node.label.hash(&mut hash);
            node.a11y.label_source.hash(&mut hash);
            node.a11y.level.hash(&mut hash);
            node.a11y.expanded.hash(&mut hash);
            node.disabled.hash(&mut hash);
            node.a11y.invalid.hash(&mut hash);
            node.a11y.secure.hash(&mut hash);
            node.a11y.checked.hash(&mut hash);
            node.a11y.controls.hash(&mut hash);
            node.a11y.orientation.hash(&mut hash);
            node.a11y.pressed.hash(&mut hash);
            node.a11y.current.hash(&mut hash);
            node.a11y.multiselectable.hash(&mut hash);
            if let Some(range) = node.a11y.range {
                range.value.to_bits().hash(&mut hash);
                range.min.to_bits().hash(&mut hash);
                range.max.to_bits().hash(&mut hash);
            }
            node.focus.enabled.hash(&mut hash);
            node.focus.order.hash(&mut hash);
            node.focus.group.hash(&mut hash);
            node.focus.selected.hash(&mut hash);
            node.focus.in_selection.hash(&mut hash);
            node.focus.cursor.hash(&mut hash);
            node.focus.step.is_some().hash(&mut hash);
            node.handlers.on_press.is_some().hash(&mut hash);
            node.window_drag.hash(&mut hash);
            node.help.hash(&mut hash);
            node.help_cut.hash(&mut hash);
            node.elided.is_some().hash(&mut hash);
            if let Some(spec) = node.text.as_deref() {
                spec.redacted.hash(&mut hash);
                let secret = spec.redacted || node.is_secure();
                secret.hash(&mut hash);
                if secret {
                    spec.content.len().hash(&mut hash);
                } else {
                    spec.content.hash(&mut hash);
                    spec.uncopied.hash(&mut hash);
                }
                for link in &spec.links {
                    link.start.hash(&mut hash);
                    link.end.hash(&mut hash);
                    link.id.hash(&mut hash);
                    link.role.hash(&mut hash);
                    link.label.hash(&mut hash);
                }
            }
            if let Some(input) = node.input.as_ref() {
                input.placeholder.hash(&mut hash);
                input.single_line.hash(&mut hash);
                input.access.is_none().hash(&mut hash);
                let secure = node.a11y.secure || input.edit.is_secure();
                let focused_editor = self.focused.as_deref() == Some(node.path.as_str());
                secure.hash(&mut hash);
                focused_editor.hash(&mut hash);
                if !secure && !focused_editor {
                    input.edit.text.hash(&mut hash);
                    input.edit.anchor.hash(&mut hash);
                    input.edit.focus.hash(&mut hash);
                } else {
                    input.edit.text.len().hash(&mut hash);
                }
            }
            self.in_scope(i).hash(&mut hash);
            self.scene.contains(node.id).hash(&mut hash);
            self.is_exiting(node.id).hash(&mut hash);
            (node.vis.opacity > 0.01).hash(&mut hash);
            (node.local.w > 0.0 && node.local.h > 0.0).hash(&mut hash);
            let rect = rect4(self.drawn_box(i));
            (rect[2] > 0.0 && rect[3] > 0.0).hash(&mut hash);
        }
        let mut lists: Vec<_> = self.virtuals.iter().collect();
        lists.sort_unstable_by_key(|(path, _)| *path);
        for (path, list) in lists {
            path.hash(&mut hash);
            list.keys.hash(&mut hash);
        }
        let signature = hash.finish();
        let changed = mirror.signature != Some(signature);
        let mut bounds = Vec::new();
        let items = if changed {
            let (items, sources) = self.collect_a11y(true);
            for (i, source) in sources.iter().enumerate() {
                if mirror.sources.get(i).is_none_or(|old| old.path != source.path || old.rect != source.rect) {
                    bounds.push(json!([source.path, source.rect]));
                }
            }
            mirror.sources = sources;
            Some(items)
        } else {
            for source in &mut mirror.sources {
                let Some(&i) = self.node_index.get(&source.primary) else { continue };
                let mut rect = rect4(self.drawn_box(i));
                for id in &source.extra {
                    let Some(&i) = self.node_index.get(id) else { continue };
                    let next = rect4(self.drawn_box(i));
                    let x = rect[0].min(next[0]);
                    let y = rect[1].min(next[1]);
                    let right = (rect[0] + rect[2]).max(next[0] + next[2]);
                    let bottom = (rect[1] + rect[3]).max(next[1] + next[3]);
                    rect = [x, y, round1(right - x), round1(bottom - y)];
                }
                if source.rect != rect {
                    source.rect = rect;
                    bounds.push(json!([source.path, rect]));
                }
            }
            None
        };
        let old_focus = mirror.focus.as_ref().map(|(path, link)| (path.as_str(), *link));
        if !changed && bounds.is_empty() && old_focus == focus && mirror.modal == modal { return None; }
        let mut update = serde_json::Map::new();
        if let Some(items) = items {
            update.insert("items".into(), Value::Array(items.iter().map(A11yItem::to_json).collect()));
        }
        update.insert("bounds".into(), Value::Array(bounds));
        update.insert("focus".into(), focus.map_or(Value::Null, |(path, link)| json!({"path": path, "link": link})));
        update.insert("modal".into(), json!(modal));
        mirror.json.clear();
        serde_json::to_writer(&mut mirror.json, &Value::Object(update)).expect("accessibility values serialize as JSON");
        mirror.signature = Some(signature);
        mirror.focus = focus.map(|(path, link)| (path.to_owned(), link));
        mirror.modal = modal;
        Some(std::str::from_utf8(&mirror.json).expect("JSON is UTF-8"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::el::a11y::CheckState;
    use serde_json::json;

    #[test]
    fn mirrors_headings_inline_links_and_buttons_and_skips_idle_frames() {
        fn view(open: &(bool, usize), _: &mut Cx<(bool, usize)>) -> El {
            let mut body = crate::rich("Read spec for details.", vec![]).font(crate::tok::type_::BODY)
                .link(crate::Link::new(5..9, "https://example.com/spec"))
                .on_link(|s: &mut (bool, usize), _, _| s.1 += 1);
            body = body.selectable(true);
            let toggle = crate::row().key("fold").role("button").label(if open.0 { "Hide details" } else { "Show details" })
                .expanded(open.0)
                .on_press(|s: &mut (bool, usize)| s.0 = !s.0)
                .child(crate::text("Details"));
            crate::col().w_full().gap(8.0)
                .child(crate::text("Release notes").role("heading").level(1).font(crate::tok::type_::TITLE))
                .child(body)
                .child(toggle)
        }
        let mut rt = Runtime::new((false, 0usize), view, (480.0, 320.0), Env::default());
        rt.frame(0.0);
        let mut mirror = A11yMirror::default();
        let first = rt.web_a11y(&mut mirror).expect("initial tree").to_owned();
        assert!(rt.web_a11y(&mut mirror).is_none(), "unchanged frame emits nothing");

        let items = rt.web_a11y_items();
        assert_eq!(items.len(), 3);
        assert_eq!((items[0].kind, items[0].level, items[0].text.as_str()), ("heading", Some(1), "Release notes"));
        assert_eq!(items[1].kind, "text");
        assert_eq!(items[1].runs.len(), 3);
        assert_eq!((items[1].runs[1].text.as_str(), items[1].runs[1].href.as_deref()), ("spec", Some("https://example.com/spec")));
        assert_eq!((items[2].kind, items[2].text.as_str(), items[2].expanded), ("button", "Show details", Some(false)));

        let btn_path = items[2].path.clone();
        let link_path = items[1].path.clone();
        assert!(rt.web_a11y_action(&json!({"kind": "a11y_press", "path": btn_path})));
        rt.frame(0.0);
        assert!(rt.state.0);
        let updated = rt.web_a11y(&mut mirror).expect("button flip updates expanded state");
        assert_ne!(first, updated);
        assert!(updated.contains("Hide details"));

        for link in [json!(-1), json!("0"), json!(1e30)] {
            assert!(!rt.web_a11y_action(&json!({"kind": "a11y_link", "path": link_path, "link": link})));
            assert!(!rt.web_a11y_action(&json!({"kind": "a11y_focus", "path": link_path, "link": link})));
        }
        assert!(!rt.web_a11y_action(&json!({"kind": "a11y_link", "path": format!("{link_path}#link-0"), "link": 0})));
        assert!(rt.web_a11y_action(&json!({"kind": "a11y_link", "path": link_path, "link": 0})));
        rt.frame(0.0);
        assert_eq!(rt.state.1, 1);
    }

    #[test]
    fn mac_meta_copy_and_select_all_work_on_selectable_text() {
        fn view(_: &(), _: &mut Cx<()>) -> El {
            crate::col().key("doc").selection_group(true)
                .child(crate::text("First paragraph").key("p1").font(crate::tok::type_::BODY).selectable(true))
                .child(crate::text("Second paragraph").key("p2").font(crate::tok::type_::BODY).selectable(true))
        }
        let mut rt = Runtime::new((), view, (400.0, 200.0), Env::default());
        let board = crate::clipboard::memory();
        rt.use_clipboard(&board);
        rt.frame(0.0);
        rt.pointer_down(12.0, 8.0);
        rt.pointer_up(12.0, 8.0);
        rt.frame(0.0);
        // Simulate macOS browser Meta+A (meta=true, ctrl=false) then Meta+C.
        rt.web_key(&json!({"kind":"key", "key":"a", "code":"KeyA", "meta":true, "ctrl":false, "shift":false, "alt":false}));
        rt.frame(0.0);
        assert_eq!(rt.copy_text().as_deref(), Some("First paragraph\nSecond paragraph"));
        rt.web_key(&json!({"kind":"key", "key":"c", "code":"KeyC", "meta":true, "ctrl":false, "shift":false, "alt":false}));
        rt.frame(0.0);
        crate::clipboard::flush_memory(&mut rt);
        assert_eq!(board.text().as_deref().map(String::as_str), Some("First paragraph\nSecond paragraph"));
    }

    #[test]
    fn editor_privacy_names_and_control_state_are_projected() {
        struct Form { edit: crate::TextEdit, secret: crate::TextEdit, readonly: crate::TextEdit }
        fn view(s: &Form, _: &mut Cx<Form>) -> El {
            let style = crate::TextStyle::new(14.0, 400.0);
            crate::col().size(400.0, 800.0)
                .child(crate::col().key("shell").label_if_missing("Account").role("textbox")
                    .child(crate::text_input(&s.edit, style).key("edit").size(200.0, 30.0)
                        .placeholder_label("Fallback").on_edit(|s: &mut Form| &mut s.edit)))
                .child(crate::text_input(&s.secret, style).key("secret").size(200.0, 30.0)
                    .secure().on_edit(|s: &mut Form| &mut s.secret))
                .child(crate::text_input(&s.readonly, style).key("readonly").size(200.0, 30.0))
                .child(crate::el().key("check").role("checkbox").label("Remember")
                    .checked(true).indeterminate(true).disabled(true).size(30.0, 30.0))
                .child(crate::row().key("choice").role("combobox").label_if_missing("Colour")
                    .help("Select a colour").size(200.0, 30.0).child(crate::text("Blue")))
                .child(crate::rich("é one two", vec![]).key("links")
                    .link(crate::Link::new(7..10, "second"))
                    .link(crate::Link::new(3..6, "first"))
                    .link(crate::Link::new(1..2, "invalid"))
                    .link(crate::Link::new(4..5, "overlap")))
                .child(crate::text("one many many many two").key("elided").font(style).w(64.0).truncate()
                    .help_when_truncated("Full copy")
                    .link(crate::Link::new(0..3, "visible")).link(crate::Link::new(19..22, "hidden")))
                .child(crate::row()
                    .child(crate::text("first").key("flow-first"))
                    .child(crate::text("second").key("flow-second")))
                .child(crate::row().key("visible-check").role("checkbox").label("Remember")
                    .child(crate::text("Remember")).child(crate::text("On this device")))
                .child(crate::row().key("hidden-check").role("switch").label("Notifications")
                    .child(crate::text("Notifications").opacity(0.0)).child(crate::text("Description")))
                .child(crate::row().key("visible-radio").role("radio").label("Blue").child(crate::text("Blue")))
                .child(crate::col().key("multi").role("group").label_if_missing("Address")
                    .child(crate::text_input(&s.readonly, style).key("multi-one").size(100.0, 20.0))
                    .child(crate::text_input(&s.readonly, style).key("multi-two").size(100.0, 20.0)))
                .child(crate::col().key("custom").role("group").label("Custom group")
                    .child(crate::text_input(&s.readonly, style).key("custom-edit").size(100.0, 20.0)))
                .child(crate::col().role("textbox").label_if_missing("Outer")
                    .child(crate::text_input(&s.readonly, style).key("explicit-edit").label("Inner").size(100.0, 20.0)))
                .child(crate::text("short").key("fitted").font(style).w(100.0).truncate()
                    .help_when_truncated("Not needed"))
        }
        let mut rt = Runtime::new(Form {
            edit: crate::TextEdit::new("a".repeat(1600)),
            secret: crate::TextEdit::new("private value"),
            readonly: crate::TextEdit::new("read only"),
        }, view, (400.0, 800.0), Env::default());
        rt.frame(0.0);
        let path = |key: &str| rt.nodes.iter().find(|n| n.key.as_deref() == Some(key)).unwrap().path.clone();
        let (edit, secret, readonly, check, choice, links) = (path("edit"), path("secret"), path("readonly"), path("check"), path("choice"), path("links"));
        let items = rt.web_a11y_items();
        let item = |path: &str| items.iter().find(|item| item.path == path).unwrap();
        assert_eq!(item(&edit).name.as_deref(), Some("Account"));
        assert!(!items.iter().any(|it| it.path == path("shell")));
        assert!(!items.iter().any(|it| it.kind == "group" && it.name.is_none() && it.tab.is_none()));
        assert_eq!(item(&path("multi")).name.as_deref(), Some("Address"));
        assert_eq!(item(&path("multi-one")).name, None);
        assert_eq!(item(&path("custom")).name.as_deref(), Some("Custom group"));
        assert_eq!(item(&path("custom-edit")).name, None);
        assert_eq!(item(&path("explicit-edit")).name.as_deref(), Some("Inner"));
        assert_eq!((item(&path("visible-check")).text.as_str(), item(&path("visible-check")).name.as_deref()),
            ("Remember On this device", None));
        assert_eq!((item(&path("hidden-check")).text.as_str(), item(&path("hidden-check")).name.as_deref()),
            ("Description", Some("Notifications")));
        assert_eq!((item(&path("visible-radio")).text.as_str(), item(&path("visible-radio")).name.as_deref()), ("Blue", None));
        assert!(item(&edit).edit.as_ref().unwrap().text.as_ref().unwrap().len() < 1600);
        assert_eq!(item(&secret).edit.as_ref().unwrap().text.as_deref(), Some(""));
        assert_eq!(item(&readonly).edit.as_ref().unwrap().text.as_deref(), Some("read only"));
        assert_eq!(item(&check).checked, Some(CheckState { checked: true, indeterminate: true }));
        assert_eq!((item(&check).text.as_str(), item(&check).name.as_deref()), ("", Some("Remember")));
        assert!(item(&check).disabled);
        assert_eq!((item(&choice).name.as_deref(), item(&choice).text.as_str()), (Some("Colour"), "Blue"));
        assert_eq!(item(&choice).to_json()["help"], "Select a colour");
        assert_eq!(item(&path("fitted")).help, None);
        let runs = &item(&links).runs;
        assert_eq!(runs.iter().map(|run| run.text.as_str()).collect::<String>(), "é one two");
        assert_eq!(runs.iter().filter_map(|run| run.link).collect::<Vec<_>>(), vec![1, 3, 0]);
        assert_eq!(runs.iter().find(|run| run.link == Some(3)).unwrap().label.as_deref(), Some("n"));
        let clipped = items.iter().find(|item| item.path.ends_with("/elided")).unwrap();
        assert_eq!(clipped.runs.iter().map(|run| run.text.as_str()).collect::<String>(), "one many many many two");
        assert_eq!(clipped.runs.iter().filter_map(|run| run.href.as_deref()).collect::<Vec<_>>(), ["visible"]);
        assert_eq!(clipped.help.as_deref(), Some("Full copy"));
        assert!(!rt.a11y_json().contains("private value"));
        assert!(items.iter().any(|it| it.text == "first"));
        assert!(items.iter().any(|it| it.text == "second"));
        assert!(!items.iter().any(|it| it.text.contains("first") && it.text.contains("second")));
        let first = rt.nodes.iter().position(|n| n.key.as_deref() == Some("flow-first")).unwrap();
        let second = rt.nodes.iter().position(|n| n.key.as_deref() == Some("flow-second")).unwrap();
        let (first_path, second_path) = (rt.nodes[first].path.clone(), rt.nodes[second].path.clone());
        rt.drawn[first] = [10.0, 220.0, 30.0, 15.0];
        rt.drawn[second] = [35.0, 220.0, 30.0, 15.0];
        let mut mirror = A11yMirror::default();
        assert!(rt.web_a11y(&mut mirror).is_some());
        rt.drawn[second][0] = 50.0;
        let update: Value = serde_json::from_str(rt.web_a11y(&mut mirror).unwrap()).unwrap();
        assert!(update.get("items").is_none());
        let bounds = update["bounds"].as_array().unwrap();
        assert!(bounds.iter().any(|entry| entry[0] == second_path && entry[1] == json!([50.0, 220.0, 30.0, 15.0])));
        assert!(!bounds.iter().any(|entry| entry[0] == first_path));
        assert!(rt.web_a11y(&mut mirror).is_none());
        rt.set_focus(Some(edit.clone()), true);
        assert_eq!(rt.web_a11y_items().iter().find(|item| item.path == edit).unwrap().edit.as_ref().unwrap().text, None);
        assert!(serde_json::from_str::<Value>(&rt.a11y_json()).unwrap().is_array());
    }

    #[test]
    fn stale_disabled_and_modal_actions_are_denied() {
        #[derive(Default)]
        struct State { disabled: bool, modal: bool, gone: bool, presses: usize }
        fn view(s: &State, _: &mut Cx<State>) -> El {
            let mut root = crate::col().size(300.0, 200.0);
            if !s.gone {
                root = root.child(crate::col().key("fading").child(
                    crate::el().key("button").role("button").label("Go")
                        .size(80.0, 30.0).focusable(true).disabled(s.disabled)
                        .on_press(|s: &mut State| s.presses += 1)));
            }
            if s.modal {
                root = root.child(crate::col().key("modal").size(100.0, 80.0)
                    .modal(false, |s: &mut State| s.modal = false)
                    .child(crate::el().role("button").label("Inside").size(50.0, 30.0)));
            }
            root
        }
        let mut rt = Runtime::new(State::default(), view, (300.0, 200.0), Env::default());
        rt.frame(0.0);
        let path = rt.nodes.iter().find(|n| n.key.as_deref() == Some("button")).unwrap().path.clone();
        let event = json!({"kind":"a11y_press", "path":path});
        let focus = json!({"kind":"a11y_focus", "path":path});
        rt.update(|s, _| s.disabled = true);
        rt.frame(0.0);
        assert!(!rt.web_a11y_action(&event));
        assert!(!rt.web_a11y_action(&focus));
        rt.update(|s, _| { s.disabled = false; s.modal = true; });
        rt.frame(0.0);
        assert!(!rt.web_a11y_action(&event));
        assert!(!rt.web_a11y_action(&focus));
        rt.update(|s, _| s.modal = false);
        rt.frame(0.0);
        let parent = rt.nodes.iter().position(|n| n.key.as_deref() == Some("fading")).unwrap();
        let mut mirror = A11yMirror::default();
        assert!(rt.web_a11y(&mut mirror).is_some());
        for opacity in [0.0, 0.01] {
            rt.nodes[parent].vis.opacity = opacity;
            assert!(!rt.web_a11y_items().iter().any(|item| item.path == path));
            assert!(!rt.web_a11y_action(&event));
            assert!(!rt.web_a11y_action(&focus));
        }
        rt.nodes[parent].vis.opacity = 0.011;
        assert!(rt.web_a11y_items().iter().any(|item| item.path == path));
        assert!(rt.web_a11y_action(&focus));
        rt.nodes[parent].vis.opacity = 0.0;
        let update: Value = serde_json::from_str(rt.web_a11y(&mut mirror).unwrap()).unwrap();
        assert!(!update["items"].as_array().unwrap().iter().any(|item| item["path"] == path));
        rt.nodes[parent].vis.opacity = 1.0;
        rt.scene.remove(rt.nodes[parent].id);
        assert!(rt.subtree_exiting(parent));
        assert!(!rt.web_a11y_items().iter().any(|item| item.path == path));
        assert!(!rt.web_a11y_action(&event));
        assert!(!rt.web_a11y_action(&focus));
        rt.update(|s, _| { s.modal = false; s.gone = true; });
        rt.frame(0.0);
        assert!(!rt.web_a11y_action(&event));
        assert!(!rt.web_a11y_action(&focus));
        assert_eq!(rt.state.presses, 0);
    }
    #[test]
    fn flattened_option_and_tab_names_keep_visible_text() {
        struct Form { edit: crate::TextEdit }
        fn view(s: &Form, _: &mut Cx<Form>) -> El {
            let style = crate::TextStyle::new(14.0, 400.0);
            crate::col().size(360.0, 1100.0)
                .child(crate::el().key("blue").role("option").label("Blue").size(160.0, 28.0).on_press(|_: &mut Form| {})
                    .child(crate::text("Blue").font(style)).child(crate::text("Primary").font(style)))
                .child(crate::el().key("named").role("option").label("Choose blue").size(160.0, 28.0).on_press(|_: &mut Form| {})
                    .child(crate::text("Blue").font(style)).child(crate::text("Support").font(style)))
                .child(crate::el().key("captioned").role("option").label("Blue").size(160.0, 28.0).on_press(|_: &mut Form| {})
                    .child(crate::text("Blue").font(style))
                    .child(crate::text("Account").role("label").font(style).size(80.0, 16.0)))
                .child(crate::el().key("tab").role("tab").label("Blue").size(80.0, 24.0).on_press(|_: &mut Form| {})
                    .child(crate::text("Blue").font(style)).child(crate::text("Extra").font(style)))
                .child(crate::el().key("plain").role("option").label("Plain").size(80.0, 24.0)
                    .child(crate::text("Plain").font(style)))
                .child(crate::el().key("nest").role("option").label("Outer").size(160.0, 28.0).on_press(|_: &mut Form| {})
                    .child(crate::text("Outer").font(style))
                    .child(crate::el().key("inner").role("button").label("Inner").size(40.0, 20.0).on_press(|_: &mut Form| {})))
                .child(crate::el().key("only").role("option").label("Only").size(80.0, 24.0).on_press(|_: &mut Form| {}))
                .child(crate::el().key("gridrow").role("row").size(160.0, 24.0).on_press(|_: &mut Form| {})
                    .child(crate::text("Ada").font(style)).child(crate::text("Open").font(style)))
                .child(crate::el().key("hidden").role("option").label("Blue").size(120.0, 28.0).on_press(|_: &mut Form| {})
                    .child(crate::text("Blue").opacity(0.0).font(style)).child(crate::text("Shown").font(style)))
                .child(crate::col().key("colour-field").size(200.0, 60.0)
                    .child(crate::text("Colour").role("label").key("caption").font(style).size(80.0, 16.0))
                    .child(crate::row().key("combo").role("combobox").label_if_missing("Colour").size(160.0, 28.0)
                        .child(crate::text("Blue").font(style))))
                .child(crate::col().key("account-field").size(200.0, 60.0)
                    .child(crate::text("Account").role("label").key("account-caption").font(style).size(80.0, 16.0))
                    .child(crate::text_input(&s.edit, style).key("account").label_if_missing("Account").size(120.0, 24.0)))
                .child(crate::el().key("meter").role("progressbar").label("Load").size(80.0, 8.0)
                    .value_range(40.0, 0.0..=100.0).invalid(true))
                .child(crate::el().key("off").role("checkbox").label("Off").checked(false).disabled(true).size(20.0, 20.0))
        }
        let mut rt = Runtime::new(Form { edit: crate::TextEdit::new("secret") }, view, (400.0, 1200.0), Env::default());
        rt.frame(0.0);
        let items = rt.web_a11y_items();
        let path = |key: &str| rt.nodes.iter().find(|n| n.key.as_deref() == Some(key)).unwrap().path.clone();
        let item = |key: &str| items.iter().find(|it| it.path == path(key)).unwrap_or_else(|| panic!("no {key}"));
        assert_eq!((item("blue").name.as_deref(), item("blue").text.as_str()), (None, "Blue Primary"));
        assert!(!items.iter().any(|it| it.parent.as_deref() == Some(item("blue").path.as_str())));
        assert_eq!((item("named").name.as_deref(), item("named").text.as_str()), (Some("Choose blue"), "Blue Support"));
        assert_eq!((item("captioned").name.as_deref(), item("captioned").text.as_str()), (None, "Blue"));
        assert!(!item("captioned").text.contains("Account"));
        assert_eq!((item("tab").name.as_deref(), item("tab").text.as_str()), (None, "Blue Extra"));
        assert_eq!((item("plain").name.as_deref(), item("plain").text.as_str()), (None, ""));
        assert!(items.iter().any(|it| it.parent.as_deref() == Some(item("plain").path.as_str()) && it.text == "Plain"));
        assert_eq!(item("nest").name.as_deref(), None);
        assert!(items.iter().any(|it| it.path == path("inner")));
        assert_eq!(item("only").name.as_deref(), Some("Only"));
        assert_eq!((item("hidden").name.as_deref(), item("hidden").text.as_str()), (Some("Blue"), "Shown"));
        assert_eq!(item("gridrow").text.as_str(), "");
        assert!(item("gridrow").pressable && item("gridrow").tab.is_none());
        assert!(items.iter().any(|it| it.parent.as_deref() == Some(item("gridrow").path.as_str()) && it.text == "Ada"));
        assert!(items.iter().any(|it| it.parent.as_deref() == Some(item("gridrow").path.as_str()) && it.text == "Open"));
        assert_eq!((item("combo").name.as_deref(), item("combo").text.as_str(), item("combo").labelled_by.as_deref()),
            (Some("Colour"), "Blue", Some(path("caption").as_str())));
        assert_eq!(item("account").name.as_deref(), Some("Account"));
        assert_eq!(item("account").labelled_by.as_deref(), Some(path("account-caption").as_str()));
        assert_eq!(item("account").edit.as_ref().unwrap().text.as_deref(), Some("secret"));
        assert_eq!(item("meter").range, Some(crate::ValueRange { value: 40.0, min: 0.0, max: 100.0 }));
        assert!(item("meter").invalid);
        assert_eq!(item("meter").to_json()["range"], json!({"value": 40.0, "min": 0.0, "max": 100.0}));
        assert_eq!(item("meter").to_json()["invalid"], true);
        assert_eq!((item("off").checked, item("off").disabled),
            (Some(CheckState { checked: false, indeterminate: false }), true));
        let info = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("meter")).unwrap();
        assert_eq!((info.range, info.invalid, info.disabled), (item("meter").range, true, false));
        let off = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("off")).unwrap();
        assert_eq!((off.checked, off.mixed, off.disabled, off.invalid), (Some(false), false, true, false));
        let tree = crate::tooling::tree_json(&rt, 0.0);
        let els = tree["elements"].as_array().unwrap();
        let meter = els.iter().find(|e| e["key"] == "meter").unwrap();
        assert_eq!(meter["range"], json!({"value": 40.0, "min": 0.0, "max": 100.0}));
        assert_eq!(meter["invalid"], true);
        let off_json = els.iter().find(|e| e["key"] == "off").unwrap();
        assert_eq!(off_json["checked"], false);
        assert_eq!(off_json["disabled"], true);
        assert!(serde_json::from_str::<serde_json::Value>(&rt.a11y_json()).unwrap().is_array());
    }

    #[test]
    fn active_descendant_follows_the_keyboard_owner_and_clears_when_it_leaves() {
        struct Rel { show_row: bool, show_option: bool, edit: crate::TextEdit }
        fn view(s: &Rel, _: &mut Cx<Rel>) -> El {
            let style = crate::TextStyle::new(14.0, 400.0);
            let mut folder = crate::col().key("folder").role("group").focus_group().label("Folder")
                .focus_selected(s.show_row).size(200.0, 28.0);
            if s.show_row {
                folder = folder.child(crate::el().key("nested-row").role("treeitem").level(8).label("Notes")
                    .focus_selected(true).focus_cursor(true).size(180.0, 18.0).child(crate::text("Notes").font(style)));
            }
            let tree = crate::col().key("files").role("tree").focusable(true).label("Files").size(220.0, 120.0)
                .child(crate::el().key("also").role("treeitem").label("Also").focus_selected(true).size(180.0, 18.0))
                .child(folder)
                .child(crate::col().key("inner").role("grid").focusable(true).size(200.0, 40.0)
                    .child(crate::el().key("inner-row").role("row").focus_selected(true).focus_cursor(true).label("Inside").size(180.0, 18.0)));
            let mut popup = crate::col().key("popup").role("dialog").size(200.0, 160.0)
                .child(crate::text_input(&s.edit, style).key("search").size(160.0, 28.0));
            if s.show_option {
                popup = popup.child(crate::col().key("opts").role("group").focus_group().label("Results").focus_selected(true)
                    .child(crate::el().key("skip").role("option").label("Skip").focusable(true).focus_selected(true).size(140.0, 20.0))
                    .child(crate::el().key("hit").role("option").label("Blue").focus_selected(true).size(160.0, 24.0)
                        .on_press(|_: &mut Rel| {}).child(crate::text("Blue").font(style)).child(crate::text("Primary").font(style))));
            }
            crate::col().size(440.0, 420.0).child(tree).child(popup)
                .child(crate::text_input(&s.edit, style).key("outside").size(160.0, 28.0))
        }
        let mut rt = Runtime::new(Rel { show_row: true, show_option: true, edit: crate::TextEdit::new("") }, view, (440.0, 500.0), Env::default());
        rt.frame(0.0);
        let path = |rt: &Runtime<Rel>, key: &str| rt.nodes.iter().find(|n| n.key.as_deref() == Some(key)).unwrap().path.clone();
        fn at<'a>(items: &'a [A11yItem], p: &str) -> &'a A11yItem {
            items.iter().find(|it| it.path == p).unwrap_or_else(|| panic!("no {p}"))
        }
        let owned = |items: &[A11yItem]| items.iter().all(|it| it.active_descendant.as_ref().is_none_or(|p| items.iter().any(|other| other.path == *p)));
        let items = rt.web_a11y_items();
        let (files, row, inner, inner_row, folder) = (path(&rt, "files"), path(&rt, "nested-row"), path(&rt, "inner"), path(&rt, "inner-row"), path(&rt, "folder"));
        assert_eq!(at(&items, &files).active_descendant.as_deref(), Some(row.as_str()));
        assert!(at(&items, &folder).active_descendant.is_none());
        assert_eq!(at(&items, &inner).active_descendant.as_deref(), Some(inner_row.as_str()));
        assert_ne!(at(&items, &files).active_descendant.as_deref(), Some(inner_row.as_str()));
        assert_eq!(at(&items, &row).level, Some(8));
        assert_eq!(at(&items, &row).to_json()["level"], 8);
        assert!(owned(&items));
        let (search, outside, hit) = (path(&rt, "search"), path(&rt, "outside"), path(&rt, "hit"));
        rt.set_focus(Some(outside.clone()), true);
        let outside_items = rt.web_a11y_items();
        assert!(at(&outside_items, &outside).active_descendant.is_none());
        rt.set_focus(Some(search.clone()), true);
        let focused = rt.web_a11y_items();
        assert!(at(&focused, &search).active_descendant.is_none(), "an editor does not guess a sibling option");
        assert!(at(&focused, &path(&rt, "opts")).active_descendant.is_none());
        assert_eq!((at(&focused, &hit).name.as_deref(), at(&focused, &hit).text.as_str()), (None, "Blue Primary"));
        assert!(owned(&focused));
        rt.update(|s, _| { s.show_row = false; s.show_option = false; });
        rt.frame(0.0);
        let search = path(&rt, "search");
        rt.set_focus(Some(search.clone()), true);
        let gone = rt.web_a11y_items();
        assert!(at(&gone, &search).active_descendant.is_none());
        assert!(at(&gone, &path(&rt, "files")).active_descendant.is_none());
        assert!(owned(&gone));
    }

    #[test]
    fn cursor_controls_and_pressable_rows_use_declared_facts() {
        struct Rel { show_c: bool, c_selected: bool, edit: crate::TextEdit, presses: u32 }
        fn view(s: &Rel, _: &mut Cx<Rel>) -> El {
            let style = crate::TextStyle::new(14.0, 400.0);
            let mut grid = crate::col().key("grid").role("grid").focusable(true).size(220.0, 80.0)
                .child(crate::el().key("a").role("row").label("A").focus_selected(true).size(180.0, 18.0))
                .child(crate::el().key("b").role("row").label("B").size(180.0, 18.0));
            if s.show_c {
                grid = grid.child(crate::el().key("c").role("row").label("C").focus_selected(s.c_selected).focus_cursor(true).size(180.0, 18.0));
            }
            let search = crate::col().key("search-shell").role("combobox").expanded(true).controls("hits").label("Find")
                .child(crate::text_input(&s.edit, style).single_line().key("search").size(160.0, 24.0).on_edit(|s: &mut Rel| &mut s.edit));
            let hits = crate::col().key("hits").role("listbox").size(160.0, 48.0)
                .child(crate::el().key("opt-a").role("option").label("A").focus_selected(true).size(140.0, 18.0))
                .child(crate::el().key("opt-c").role("option").label("C").focus_cursor(true).size(140.0, 18.0));
            crate::col().size(480.0, 360.0)
                .child(grid)
                .child(search)
                .child(hits)
                .child(crate::text_input(&s.edit, style).key("other").size(160.0, 24.0))
                .child(crate::el().key("pane").size(80.0, 40.0))
                .child(crate::el().key("grip").role("separator").size(6.0, 40.0).focusable(true)
                    .controls("pane").orientation(crate::Orientation::Vertical).value_range(20.0, 10.0..=40.0))
                .child(crate::el().key("stale").role("separator").size(6.0, 16.0).controls("missing"))
                .child(crate::col().key("left").child(crate::el().key("dup").size(10.0, 10.0)))
                .child(crate::col().key("right").child(crate::el().key("dup").size(10.0, 10.0)))
                .child(crate::el().key("dup-owner").role("separator").size(6.0, 16.0).controls("dup"))
                .child(crate::el().key("press-row").role("row").size(160.0, 24.0).on_press(|s: &mut Rel| s.presses += 1)
                    .child(crate::el().key("cell-a").role("gridcell").size(70.0, 20.0).child(crate::text("Ada").font(style)))
                    .child(crate::el().key("cell-b").role("gridcell").size(70.0, 20.0).child(crate::text("Open").font(style))))
        }
        let mut rt = Runtime::new(Rel { show_c: true, c_selected: false, edit: crate::TextEdit::new(""), presses: 0 }, view, (480.0, 400.0), Env::default());
        rt.frame(0.0);
        let path = |rt: &Runtime<Rel>, key: &str| rt.nodes.iter().find(|n| n.key.as_deref() == Some(key)).unwrap().path.clone();
        let items = rt.web_a11y_items();
        fn at<'a>(items: &'a [A11yItem], p: &str) -> &'a A11yItem {
            items.iter().find(|it| it.path == p).unwrap_or_else(|| panic!("no {p}"))
        }
        let (grid, c, a) = (path(&rt, "grid"), path(&rt, "c"), path(&rt, "a"));
        assert_eq!(at(&items, &grid).active_descendant.as_deref(), Some(c.as_str()));
        let info = rt.elements();
        let cursor = info.iter().find(|e| e.key.as_deref() == Some("c")).unwrap();
        let selected = info.iter().find(|e| e.key.as_deref() == Some("a")).unwrap();
        assert!(cursor.item_cursor && !cursor.item_selected);
        assert!(selected.item_selected && !selected.item_cursor);
        rt.update(|s, _| s.show_c = false);
        rt.frame(0.0);
        let hidden = rt.web_a11y_items();
        assert!(at(&hidden, &path(&rt, "grid")).active_descendant.is_none());
        rt.update(|s, _| { s.show_c = true; s.c_selected = true; });
        rt.frame(0.0);
        let search = path(&rt, "search");
        rt.set_focus(Some(search.clone()), true);
        let focused = rt.web_a11y_items();
        let editor = at(&focused, &search);
        assert_eq!(editor.kind, "combobox");
        assert_eq!(editor.expanded, Some(true));
        assert_eq!(editor.controls.as_deref(), Some(path(&rt, "hits").as_str()));
        assert!(editor.edit.as_ref().is_some_and(|e| e.single));
        assert!(!focused.iter().any(|it| it.path == path(&rt, "search-shell")));
        assert_eq!(editor.active_descendant.as_deref(), Some(path(&rt, "opt-c").as_str()));
        assert!(at(&focused, &path(&rt, "other")).active_descendant.is_none());
        assert!(at(&focused, &path(&rt, "other")).controls.is_none());
        let grip = at(&focused, &path(&rt, "grip"));
        assert_eq!(grip.controls.as_deref(), Some(path(&rt, "pane").as_str()));
        assert_eq!(grip.orientation, Some(crate::Orientation::Vertical));
        assert!(focused.iter().any(|it| it.path == path(&rt, "pane")));
        assert!(at(&focused, &path(&rt, "stale")).controls.is_none());
        assert!(at(&focused, &path(&rt, "dup-owner")).controls.is_none());
        let row = at(&focused, &path(&rt, "press-row"));
        assert!(row.pressable && row.tab.is_none() && row.text.is_empty());
        assert!(focused.iter().any(|it| it.path == path(&rt, "cell-a") && it.parent.as_deref() == Some(row.path.as_str())));
        assert!(rt.web_a11y_action(&json!({"kind": "a11y_press", "path": row.path})));
        assert!(!rt.web_a11y_action(&json!({"kind": "a11y_focus", "path": row.path})));
        rt.frame(0.0);
        assert_eq!(rt.state.presses, 1);
        let _ = (a, c);
    }

    #[test]
    fn a_pressable_that_contains_a_pressable_keeps_both_nodes() {
        fn view(_: &(), _: &mut Cx<()>) -> El {
            crate::col().size(240.0, 80.0).child(
                crate::el().key("today").role("button").label("Today").expanded(true).focusable(true)
                    .size(220.0, 48.0)
                    .on_press(|_: &mut ()| {})
                    .child(crate::el().key("add").role("button").label("Add").focusable(true)
                        .size(28.0, 28.0)
                        .on_press(|_: &mut ()| {})),
            )
        }
        let mut rt = Runtime::new((), view, (240.0, 80.0), Env::default());
        rt.frame(0.0);
        let items = rt.web_a11y_items();
        let path = |key: &str| rt.nodes.iter().find(|n| n.key.as_deref() == Some(key)).unwrap().path.clone();
        let today = items.iter().find(|it| it.path == path("today")).expect("outer pressable");
        let add = items.iter().find(|it| it.path == path("add")).expect("inner pressable");
        assert_eq!(today.kind, "button");
        assert_eq!(today.expanded, Some(true));
        assert!(today.name.as_deref() == Some("Today") || today.text == "Today");
        assert!(add.name.as_deref() == Some("Add") || add.text == "Add");
        assert_eq!(add.parent.as_deref(), Some(today.path.as_str()));
        assert!(today.tab.is_some() && add.tab.is_some());
    }

    #[test]
    fn a_checkbox_keeps_a_link_in_its_label() {
        fn view(_: &(), _: &mut Cx<()>) -> El {
            crate::col().key("sheet").size(320.0, 160.0)
                .modal(false, |_: &mut ()| {})
                .child(crate::el().key("agree").role("checkbox").label("Agree").size(280.0, 40.0)
                    .child(crate::rich("Read the terms", vec![]).key("copy")
                        .link(crate::Link::new(9..14, "terms"))))
        }
        let mut rt = Runtime::new((), view, (360.0, 200.0), Env::default());
        rt.frame(0.0);
        let items = rt.web_a11y_items();
        let path = |key: &str| rt.nodes.iter().find(|n| n.key.as_deref() == Some(key)).unwrap().path.clone();
        let agree = items.iter().find(|it| it.path == path("agree")).expect("checkbox");
        let copy = items.iter().find(|it| it.path == path("copy")).expect("label text");
        assert_eq!(copy.parent.as_deref(), Some(agree.path.as_str()));
        let link = copy.runs.iter().find(|run| run.href.as_deref() == Some("terms")).expect("terms link");
        assert_eq!(link.text, "terms");
        assert!(link.tab.is_some());
        assert!(rt.web_a11y_action(&json!({"kind": "a11y_focus", "path": copy.path, "link": link.link})));
    }

    #[test]
    fn a_slider_inside_a_pressable_keeps_its_node() {
        fn view(_: &(), _: &mut Cx<()>) -> El {
            crate::el().key("volume").role("button").label("Volume").size(220.0, 48.0)
                .on_press(|_: &mut ()| {})
                .child(crate::el().key("level").role("slider").size(160.0, 20.0)
                    .value_range(1.0, 0.0..=10.0)
                    .on_step(|_: &mut (), _: i32, _| {}))
        }
        let mut rt = Runtime::new((), view, (240.0, 80.0), Env::default());
        rt.frame(0.0);
        let items = rt.web_a11y_items();
        let path = |key: &str| rt.nodes.iter().find(|n| n.key.as_deref() == Some(key)).unwrap().path.clone();
        let volume = items.iter().find(|it| it.path == path("volume")).expect("pressable");
        let level = items.iter().find(|it| it.path == path("level")).expect("slider");
        assert_eq!(level.kind, "slider");
        assert_eq!(level.parent.as_deref(), Some(volume.path.as_str()));
        assert!(level.tab.is_some());
        assert!(level.range.is_some());
    }

    #[test]
    fn a_grid_sets_selected_only_on_items_in_a_selection() {
        fn view(_: &(), _: &mut Cx<()>) -> El {
            crate::col().key("grid").role("grid").multiselectable(true).size(240.0, 100.0)
                .child(crate::el().key("header").role("row").label("Name").size(200.0, 20.0))
                .child(crate::el().key("data").role("row").label("Ada").focus_selected(false).size(200.0, 20.0))
                .child(crate::el().key("cell").role("gridcell").label("Ada").size(80.0, 20.0))
                .child(crate::el().key("picked").role("gridcell").label("Ada").focus_selected(true).size(80.0, 20.0))
        }
        let mut rt = Runtime::new((), view, (260.0, 140.0), Env::default());
        rt.frame(0.0);
        let items = rt.web_a11y_items();
        let path = |key: &str| rt.nodes.iter().find(|n| n.key.as_deref() == Some(key)).unwrap().path.clone();
        let item = |key: &str| items.iter().find(|it| it.path == path(key)).unwrap_or_else(|| panic!("no {key}"));
        assert!(item("header").selected.is_none());
        assert!(item("header").to_json().get("selected").is_none());
        assert_eq!(item("data").selected, Some(false));
        assert_eq!(item("data").to_json()["selected"], false);
        assert!(item("cell").selected.is_none());
        assert!(item("cell").to_json().get("selected").is_none());
        assert_eq!(item("picked").selected, Some(true));
        assert_eq!(item("picked").to_json()["selected"], true);
        assert!(item("grid").multiselectable);
        assert_eq!(item("grid").to_json()["multiselectable"], true);
    }

    #[test]
    fn a_virtual_row_reports_its_place_in_the_full_set() {
        fn view(_: &(), _: &mut Cx<()>) -> El {
            crate::col().key("viewport").size(200.0, 200.0)
                .scroll(crate::ScrollSpec { stick_bottom: false })
                .child(crate::col().key("rows").w_full().growth(crate::GrowthSpec::default())
                    .virtual_rows(1000, 20.0,
                        |_: &(), i| std::borrow::Cow::Owned(format!("r{i}")),
                        |_: &(), i| crate::el().w_full().h(20.0).role("row").focus_selected(i == 0)))
        }
        let mut rt = Runtime::new((), view, (200.0, 200.0), Env::default());
        rt.frame(0.0);
        let items = rt.web_a11y_items();
        let rows: Vec<_> = items.iter().filter(|it| it.kind == "row").collect();
        assert!(!rows.is_empty());
        assert!(rows.len() < 1000, "the mirror lists the mounted window, not every row");
        assert!(rows.iter().all(|row| row.set_size == Some(1000) && row.pos_in_set.is_some_and(|n| n >= 1)));
        let first = rows.iter().find(|row| row.pos_in_set == Some(1)).expect("the first row is mounted");
        assert_eq!(first.to_json()["set_size"], 1000);
        assert_eq!(first.to_json()["pos_in_set"], 1);
    }

    #[test]
    fn a_heading_without_a_level_is_level_two() {
        fn view(_: &(), _: &mut Cx<()>) -> El {
            crate::col().size(200.0, 80.0)
                .child(crate::text("Title").key("plain").role("heading"))
                .child(crate::text("Chapter").key("third").role("heading").level(3))
                .child(crate::text("Deep").key("deep").role("heading").level(9))
        }
        let mut rt = Runtime::new((), view, (200.0, 120.0), Env::default());
        rt.frame(0.0);
        let items = rt.web_a11y_items();
        let path = |key: &str| rt.nodes.iter().find(|n| n.key.as_deref() == Some(key)).unwrap().path.clone();
        let item = |key: &str| items.iter().find(|it| it.path == path(key)).unwrap();
        assert_eq!(item("plain").level, Some(2));
        assert_eq!(item("plain").to_json()["level"], 2);
        assert_eq!(item("third").level, Some(3));
        assert_eq!(item("deep").level, Some(6));
        let info = |key: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).unwrap();
        assert_eq!((info("plain").level, info("third").level, info("deep").level), (Some(2), Some(3), Some(6)));
    }

    #[test]
    fn a_commit_that_only_changes_colour_does_not_reserialize_semantics() {
        fn view(_: &u32, _: &mut Cx<u32>) -> El {
            crate::el().key("swatch").role("button").label("Swatch").focusable(true).size(40.0, 40.0)
        }
        let mut rt = Runtime::new(0u32, view, (80.0, 80.0), Env::default());
        rt.frame(0.0);
        let mut mirror = A11yMirror::default();
        assert!(rt.web_a11y(&mut mirror).is_some());
        let i = rt.nodes.iter().position(|n| n.key.as_deref() == Some("swatch")).unwrap();
        rt.nodes[i].vis.fill = crate::Color::WHITE;
        assert!(rt.web_a11y(&mut mirror).is_none());
        rt.nodes[i].cold_mut().label = Some("Renamed".into());
        let update: Value = serde_json::from_str(rt.web_a11y(&mut mirror).unwrap()).unwrap();
        assert!(update["items"].as_array().unwrap().iter().any(|item| item["text"] == "Renamed" || item["name"] == "Renamed"));
    }

    #[test]
    fn the_inspector_reports_the_computed_accessible_name() {
        struct Form { edit: crate::TextEdit }
        fn view(s: &Form, _: &mut Cx<Form>) -> El {
            let style = crate::TextStyle::new(14.0, 400.0);
            crate::col().size(240.0, 120.0)
                .child(crate::col().key("shell").label_if_missing("Account").role("textbox")
                    .child(crate::text_input(&s.edit, style).key("edit").size(180.0, 28.0)))
                .child(crate::el().key("fold").role("button").label("Show details").size(160.0, 28.0)
                    .on_press(|_: &mut Form| {})
                    .child(crate::text("Details").font(style)))
        }
        let mut rt = Runtime::new(Form { edit: crate::TextEdit::new("ada") }, view, (240.0, 140.0), Env::default());
        rt.frame(0.0);
        let info = |key: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).unwrap();
        assert_eq!(info("edit").name.as_deref(), Some("Account"));
        assert_eq!(info("edit").label.as_deref(), None);
        assert_eq!(info("fold").name.as_deref(), Some("Show details"));
        assert!(info("shell").name.is_none(), "a combobox shell is not its own object");
        let tree = crate::tooling::tree_json(&rt, 0.0);
        let edit = tree["elements"].as_array().unwrap().iter().find(|e| e["key"] == "edit").unwrap();
        assert_eq!(edit["name"], "Account");
    }

    #[test]
    fn a_toggle_reports_pressed_and_a_current_item_reports_current() {
        fn view(_: &(), _: &mut Cx<()>) -> El {
            crate::col().size(200.0, 80.0)
                .child(crate::el().key("bold").role("button").label("Bold").pressed(false).size(80.0, 28.0)
                    .on_press(|_: &mut ()| {}))
                .child(crate::el().key("here").role("link").label("Home").current(crate::Current::Page).size(80.0, 28.0))
        }
        let mut rt = Runtime::new((), view, (200.0, 100.0), Env::default());
        rt.frame(0.0);
        let items = rt.web_a11y_items();
        let path = |key: &str| rt.nodes.iter().find(|n| n.key.as_deref() == Some(key)).unwrap().path.clone();
        let item = |key: &str| items.iter().find(|it| it.path == path(key)).unwrap();
        assert_eq!(item("bold").pressed, Some(false));
        assert_eq!(item("bold").to_json()["pressed"], false);
        assert_eq!(item("here").current, Some(crate::Current::Page));
        assert_eq!(item("here").to_json()["current"], "page");
        assert!(item("bold").to_json().get("current").is_none());
    }
}
