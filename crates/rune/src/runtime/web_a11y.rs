//! Semantic DOM projection and native editor contexts for the retained scene.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
use crate::el::LabelSource;
use std::hash::{Hash, Hasher};

use serde_json::{Value, json};

use super::*;

/// Read at most 512 UTF-16 units on each side, without scanning the document prefix.
pub(super) fn context_window(edit: &crate::TextEdit) -> (usize, &str, u32, u32) {
    let (a, b) = edit.range();
    let (raw_anchor, raw_focus) = if edit.anchor <= edit.focus { (a, b) } else { (b, a) };
    let mut start = a;
    let mut units = 0;
    for (i, ch) in edit.text[..a].char_indices().rev() {
        if units + ch.len_utf16() > 512 { break; }
        units += ch.len_utf16();
        start = i;
    }
    let mut end = b;
    let mut units = 0;
    for ch in edit.text[b..].chars() {
        if units + ch.len_utf16() > 512 { break; }
        units += ch.len_utf16();
        end += ch.len_utf8();
    }
    let anchor = edit.text[start..raw_anchor].encode_utf16().count() as u32;
    let focus = edit.text[start..raw_focus].encode_utf16().count() as u32;
    (start, &edit.text[start..end], anchor, focus)
}

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
    pub parent: Option<String>,
    pub tab: Option<i32>,
    pub disabled: bool,
    pub checked: Option<&'static str>,
    pub selected: bool,
    pub help: Option<String>,
    pub modal: bool,
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
        obj.insert("k".into(), json!(self.kind));
        obj.insert("p".into(), json!(self.path));
        obj.insert("r".into(), json!(self.rect));
        if self.font_size != 0.0 { obj.insert("s".into(), json!(self.font_size)); }
        if let Some(l) = self.level { obj.insert("l".into(), json!(l)); }
        if let Some(x) = self.expanded { obj.insert("x".into(), json!(x)); }
        if !self.text.is_empty() { obj.insert("t".into(), json!(self.text)); }
        if !self.runs.is_empty() {
            let runs: Vec<Value> = self.runs.iter().map(|r| {
                let mut ro = serde_json::Map::new();
                ro.insert("t".into(), json!(r.text));
                if let Some(h) = &r.href { ro.insert("h".into(), json!(h)); }
                if let Some(p) = &r.path { ro.insert("p".into(), json!(p)); }
                if let Some(i) = r.link { ro.insert("i".into(), json!(i)); }
                if let Some(role) = r.role { ro.insert("role".into(), json!(role)); }
                if let Some(label) = &r.label { ro.insert("label".into(), json!(label)); }
                if let Some(tab) = r.tab { ro.insert("tab".into(), json!(tab)); }
                Value::Object(ro)
            }).collect();
            obj.insert("u".into(), Value::Array(runs));
        }
        if let Some(name) = &self.name { obj.insert("name".into(), json!(name)); }
        if let Some(parent) = &self.parent { obj.insert("parent".into(), json!(parent)); }
        if let Some(tab) = self.tab { obj.insert("tab".into(), json!(tab)); }
        if self.disabled { obj.insert("disabled".into(), json!(true)); }
        if let Some(checked) = self.checked { obj.insert("checked".into(), json!(checked)); }
        if self.selected { obj.insert("selected".into(), json!(true)); }
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

fn round1(v: f32) -> f32 { (v * 10.0).round() / 10.0 }

fn rect4(b: [f32; 4]) -> [f32; 4] {
    [round1(b[0]), round1(b[1]), round1((b[2] - b[0]).max(0.0)), round1((b[3] - b[1]).max(0.0))]
}

fn clean_slice(spec: &crate::el::TextSpec, a: usize, b: usize) -> String {
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

    fn a11y_item(&self, i: usize, kind: &'static str, parent: Option<String>, name: Option<String>, tab: Option<i32>) -> A11yItem {
        let n = &self.nodes[i];
        A11yItem {
            kind, path: n.path.clone(), rect: rect4(self.drawn_box(i)), font_size: 0.0,
            level: None, expanded: n.a11y.expanded, text: String::new(), runs: Vec::new(),
            name, parent, tab,
            disabled: n.a11y.disabled,
            checked: n.a11y.checked.map(|s| if s.indeterminate { "mixed" } else if s.checked { "true" } else { "false" }),
            selected: n.focus.selected, help: n.help.clone().filter(|_| !n.help_cut || n.elided.is_some()),
            modal: self.active_modal() == Some(i), edit: None,
        }
    }

    /// Collect semantic DOM items for the current laid-out scene.
    pub fn a11y_json(&self) -> String {
        let values: Vec<Value> = self.web_a11y_items().iter().map(A11yItem::to_json).collect();
        serde_json::to_string(&values).unwrap_or_else(|_| "[]".into())
    }

    pub(crate) fn web_a11y_items(&self) -> Vec<A11yItem> {
        self.collect_a11y(false).0
    }

    fn collect_a11y(&self, track: bool) -> (Vec<A11yItem>, Vec<A11ySource>) {
        let mut out: Vec<A11yItem> = Vec::new();
        let mut sources = Vec::new();
        let mut last_flow_parent: Option<usize> = None;
        let visible = |i: usize| self.in_scope(i) && !self.subtree_exiting(i) && !self.subtree_unmounted(i)
            && std::iter::successors(Some(i), |&j| self.nodes[j].parent).all(|j| self.nodes[j].vis.opacity > 0.01);
        let owners = self.input_owners(visible);
        let mut tabbable = vec![false; self.nodes.len()];
        for i in self.tab_order() { tabbable[i] = true; }
        let mut labels: Vec<_> = self.nodes.iter().map(|n| n.label.as_deref()
            .filter(|s| !s.trim().is_empty()).map(|s| (s, n.a11y.label_source))).collect();
        for (i, n) in self.nodes.iter().enumerate().rev() {
            if n.input.is_none() && (n.role == "textbox" || n.a11y.label_source == LabelSource::Composed)
                && let Some(label) = labels[i] && let Some(editor) = owners[i]
                && labels[editor].is_none_or(|(_, source)| source < label.1)
            {
                labels[editor] = Some(label);
            }
        }
        let mut retained: Vec<Option<String>> = vec![None; self.nodes.len()];
        let mut i = 0;
        while i < self.nodes.len() {
            let n = &self.nodes[i];
            let end = self.subtree_end(i);
            if !visible(i) || matches!(n.role, "none" | "presentation") {
                retained[i] = n.parent.and_then(|p| retained[p].clone());
                i += 1;
                last_flow_parent = None;
                continue;
            }
            let parent = n.parent.and_then(|p| retained[p].clone());
            let tab = self.can_focus(i).then_some(if tabbable[i] { n.focus.order } else { -1 });
            if let Some(input) = n.input.as_ref() {
                let secure = n.a11y.secure || input.edit.is_secure();
                let readonly = input.access.is_none();
                let focused = !readonly && self.focused.as_deref() == Some(n.path.as_str());
                let (base, text, anchor, focus) = if secure || focused { (0, "", 0, 0) }
                    else if readonly { (0, input.edit.text.as_str(), 0, 0) }
                    else { context_window(&input.edit) };
                let mut item = self.a11y_item(i, "textbox", parent, labels[i].map(|(s, _)| s.to_owned()), tab);
                item.edit = Some(A11yEdit { text: (!focused).then(|| text.to_owned()), base, anchor, focus,
                    secure, readonly, single: input.single_line, placeholder: input.placeholder.clone() });
                A11yItem::record_source(&mut sources, track, &item, n.id);
                out.push(item);
                i = end;
                last_flow_parent = None;
                continue;
            }
            let b = self.drawn_box(i);
            let rect = rect4(b);
            let semantic_kind = if self.active_modal() == Some(i) && !matches!(n.role, "dialog" | "alertdialog") {
                Some("dialog")
            } else { match n.role {
                "group" if labels[i].is_some() || tab.is_some() || n.focus.group => Some("group"),
                "textbox" => Some("group"),
                "text" | "heading" | "button" | "group" => None,
                role => Some(role),
            }};
            let button_like = (n.role == "button" || n.on_press.is_some()) && !n.window_drag;
            let nested_press = (i + 1..end).any(|j| self.nodes[j].on_press.is_some()
                || self.nodes[j].role == "button" || self.nodes[j].input.is_some());
            if let Some(kind) = semantic_kind {
                let mut item = self.a11y_item(i, kind, parent.clone(), labels[i].map(|(s, _)| s.to_owned()), tab);
                if let Some(spec) = n.text.as_deref().filter(|t| !t.redacted) {
                    item.text = clean_slice(spec, 0, spec.content.len());
                }
                if kind == "combobox" {
                    item.text = self.descendant_label(i + 1, end);
                } else if matches!(kind, "checkbox" | "radio" | "switch" | "tab" | "option" | "menuitem"
                    | "menuitemcheckbox" | "menuitemradio") && item.name.is_none() {
                    let label = self.descendant_label(i + 1, end);
                    if !label.is_empty() { item.name = Some(label); }
                }
                retained[i] = Some(n.path.clone());
                A11yItem::record_source(&mut sources, track, &item, n.id);
                out.push(item);
                last_flow_parent = None;
                if kind == "combobox" || (button_like && !nested_press) {
                    i = end;
                    continue;
                }
                i += 1;
                continue;
            } else if button_like && !nested_press && rect[2] > 0.0 && rect[3] > 0.0 {
                let label = n.label.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| self.descendant_label(i, end));
                if !label.is_empty() {
                    let mut item = self.a11y_item(i, "button", parent.clone(), None, tab);
                    item.text = label;
                    A11yItem::record_source(&mut sources, track, &item, n.id);
                    out.push(item);
                    i = end;
                    last_flow_parent = None;
                    continue;
                }
            }
            if let Some(spec) = n.text.as_deref().filter(|t| !t.redacted && rect[2] > 0.0 && rect[3] > 0.0) {
                let runs = text_runs(&n.path, spec, tab, |k| self.allowed(i) && self.link_eligible(i, k));
                if !runs.is_empty() {
                    let is_heading = n.role == "heading";
                    // Flowed word-by-word paragraphs (`pr_chips::flow`) put sibling words (or 1-word
                    // wrapper rows) inside one flex-wrap line row: merge consecutive inline text runs
                    // sharing that flow container so screen readers and Find-in-page see whole sentences.
                    let flow_group = n.parent.and_then(|p| {
                        if self.nodes[p].key.as_deref().is_none() && self.nodes[p].parent.is_some()
                            && self.nodes[i].key.as_deref().is_some_and(|k| k.starts_with("flow-"))
                        {
                            self.nodes[p].parent
                        } else if self.nodes[i].key.as_deref().is_some_and(|k| k.starts_with("flow-")) {
                            Some(p)
                        } else {
                            None
                        }
                    });
                    if !is_heading && flow_group.is_some() && flow_group == last_flow_parent
                        && let Some(prev) = out.last_mut().filter(|it| it.kind == "text")
                    {
                        let px1 = prev.rect[0] + prev.rect[2];
                        let py1 = prev.rect[1] + prev.rect[3];
                        let nx0 = prev.rect[0].min(rect[0]);
                        let ny0 = prev.rect[1].min(rect[1]);
                        let nx1 = px1.max(rect[0] + rect[2]);
                        let ny1 = py1.max(rect[1] + rect[3]);
                        prev.rect = [nx0, ny0, round1(nx1 - nx0), round1(ny1 - ny0)];
                        let needs_space = prev.runs.last().is_some_and(|r| !r.text.ends_with(char::is_whitespace))
                            && runs.first().is_some_and(|r| !r.text.starts_with(char::is_whitespace) && !r.text.starts_with([')', ']', '.', ',', ';', ':']));
                        if needs_space {
                            if let Some(last) = prev.runs.last_mut().filter(|r| r.href.is_none()) {
                                last.text.push(' ');
                            } else {
                                prev.runs.push(A11yRun::plain(" ".into()));
                            }
                        }
                        if runs.len() == 1 && runs[0].href.is_none()
                            && let Some(last) = prev.runs.last_mut().filter(|r| r.href.is_none())
                        {
                            last.text.push_str(&runs[0].text);
                        } else {
                            prev.runs.extend(runs);
                        }
                        if track && let Some(source) = sources.last_mut() {
                            source.extra.push(n.id);
                            source.rect = prev.rect;
                        }
                        i += 1;
                        continue;
                    }
                    let (plain, rich_runs) = if runs.len() == 1 && runs[0].href.is_none() && flow_group.is_none() {
                        (runs.into_iter().next().unwrap().text, Vec::new())
                    } else {
                        (String::new(), runs)
                    };
                    let mut item = self.a11y_item(i, if is_heading { "heading" } else { "text" }, parent.clone(),
                        labels[i].map(|(s, _)| s.to_owned()), tab);
                    item.font_size = round1(spec.style.size);
                    item.level = is_heading.then(|| n.a11y.level.unwrap_or(2).clamp(1, 6));
                    item.expanded = None;
                    item.text = plain;
                    item.runs = rich_runs;
                    A11yItem::record_source(&mut sources, track, &item, n.id);
                    out.push(item);
                    last_flow_parent = if is_heading { None } else { flow_group };
                }
            } else if n.role == "heading" && let Some(lbl) = n.label.clone().filter(|s| !s.trim().is_empty()) {
                let mut item = self.a11y_item(i, "heading", parent, None, tab);
                item.font_size = 16.0;
                item.level = Some(n.a11y.level.unwrap_or(2).clamp(1, 6));
                item.expanded = None;
                item.text = lbl;
                A11yItem::record_source(&mut sources, track, &item, n.id);
                out.push(item);
                last_flow_parent = None;
            }
            if retained[i].is_none() { retained[i] = n.parent.and_then(|p| retained[p].clone()); }
            i += 1;
        }
        (out, sources)
    }

    /// Deliver semantic changes independently from geometry and focus changes.
    pub(crate) fn web_a11y<'a>(&self, mirror: &'a mut A11yMirror) -> Option<&'a str> {
        let focus_index = self.focused_index().filter(|&i| self.in_scope(i));
        let focus = focus_index.map(|i| (self.nodes[i].path.as_str(), self.focused_link_index(i)));
        let modal = self.active_modal().is_some();
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        self.commits.hash(&mut hash);
        focus.hash(&mut hash);
        modal.hash(&mut hash);
        for (i, node) in self.nodes.iter().enumerate() {
            node.id.hash(&mut hash);
            node.parent.hash(&mut hash);
            node.live_text_seen.hash(&mut hash);
            self.in_scope(i).hash(&mut hash);
            self.scene.contains(node.id).hash(&mut hash);
            self.is_exiting(node.id).hash(&mut hash);
            (node.vis.opacity > 0.01).hash(&mut hash);
            let rect = rect4(self.drawn_box(i));
            (rect[2] > 0.0 && rect[3] > 0.0).hash(&mut hash);
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

    /// DOM presence does not authorize an action on a changed scene.
    pub(crate) fn web_a11y_action(&mut self, event: &serde_json::Value) -> bool {
        if event["kind"] == "a11y_blur" {
            if self.set_focus(None, true) { self.dirty = true; }
            return true;
        }
        let Some(path) = event["path"].as_str() else { return false };
        let Some(i) = self.live(path) else { return false };
        if !self.allowed(i) || self.subtree_exiting(i) || self.subtree_unmounted(i)
            || std::iter::successors(Some(i), |&j| self.nodes[j].parent).any(|j| self.nodes[j].vis.opacity <= 0.01)
        { return false; }
        let rect = rect4(self.drawn_box(i));
        if rect[2] <= 0.0 || rect[3] <= 0.0 { return false; }
        match event["kind"].as_str() {
            Some("a11y_focus") => {
                if !event["link"].is_null() {
                    let Some(k) = event["link"].as_u64().and_then(|n| usize::try_from(n).ok()) else { return false };
                    if !self.link_eligible(i, k) { return false; }
                    return self.focus_link(i, k);
                }
                if !self.can_focus(i) { return false; }
                if self.set_focus(Some(path.to_owned()), true) { self.dirty = true; }
                self.pending.scroll_to.push((None, path.to_owned()));
                true
            }
            Some("a11y_press") if self.nodes[i].on_press.is_some() => {
                self.queue.push(Action::Press(path.to_owned(), crate::PointerButton::Primary));
                true
            }
            Some("a11y_link") => {
                let Some(k) = event["link"].as_u64().and_then(|n| usize::try_from(n).ok()) else { return false };
                if !self.link_eligible(i, k) || self.nodes[i].text.as_ref().is_none_or(|t| t.on_link.is_none()) { return false; }
                self.queue.push(Action::Link(path.to_owned(), k));
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
            crate::col().size(400.0, 300.0)
                .child(crate::col().label_if_missing("Account").role("textbox")
                    .child(crate::text_input(&s.edit, style).key("edit").size(200.0, 30.0)
                        .fallback_label("Fallback").on_edit(|s: &mut Form| &mut s.edit)))
                .child(crate::text_input(&s.secret, style).key("secret").size(200.0, 30.0)
                    .secure().on_edit(|s: &mut Form| &mut s.secret))
                .child(crate::text_input(&s.readonly, style).key("readonly").size(200.0, 30.0))
                .child(crate::el().key("check").role("checkbox").label("Remember")
                    .checked(true).indeterminate(true).disabled(true).size(30.0, 30.0))
                .child(crate::row().key("choice").role("combobox").fallback_label("Colour")
                    .size(200.0, 30.0).child(crate::text("Blue")))
                .child(crate::rich("é one two", vec![]).key("links")
                    .link(crate::Link::new(7..10, "second"))
                    .link(crate::Link::new(3..6, "first"))
                    .link(crate::Link::new(1..2, "invalid"))
                    .link(crate::Link::new(4..5, "overlap")))
                .child(crate::text("one many many many two").key("elided").font(style).w(64.0).truncate()
                    .link(crate::Link::new(0..3, "visible")).link(crate::Link::new(19..22, "hidden")))
                .child(crate::row()
                    .child(crate::text("first").key("flow-first"))
                    .child(crate::text("second").key("flow-second")))
        }
        let mut rt = Runtime::new(Form {
            edit: crate::TextEdit::new("a".repeat(1600)),
            secret: crate::TextEdit::new("private value"),
            readonly: crate::TextEdit::new("read only"),
        }, view, (400.0, 300.0), Env::default());
        rt.frame(0.0);
        let path = |key: &str| rt.nodes.iter().find(|n| n.key.as_deref() == Some(key)).unwrap().path.clone();
        let (edit, secret, readonly, check, choice, links) = (path("edit"), path("secret"), path("readonly"), path("check"), path("choice"), path("links"));
        let items = rt.web_a11y_items();
        let item = |path: &str| items.iter().find(|item| item.path == path).unwrap();
        assert_eq!(item(&edit).name.as_deref(), Some("Account"));
        assert!(item(&edit).edit.as_ref().unwrap().text.as_ref().unwrap().len() < 1600);
        assert_eq!(item(&secret).edit.as_ref().unwrap().text.as_deref(), Some(""));
        assert_eq!(item(&readonly).edit.as_ref().unwrap().text.as_deref(), Some("read only"));
        assert_eq!(item(&check).checked, Some("mixed"));
        assert!(item(&check).disabled);
        assert_eq!((item(&choice).name.as_deref(), item(&choice).text.as_str()), (Some("Colour"), "Blue"));
        let runs = &item(&links).runs;
        assert_eq!(runs.iter().map(|run| run.text.as_str()).collect::<String>(), "é one two");
        assert_eq!(runs.iter().filter_map(|run| run.link).collect::<Vec<_>>(), vec![1, 3, 0]);
        assert_eq!(runs.iter().find(|run| run.link == Some(3)).unwrap().label.as_deref(), Some("n"));
        let clipped = items.iter().find(|item| item.path.ends_with("/elided")).unwrap();
        assert_eq!(clipped.runs.iter().map(|run| run.text.as_str()).collect::<String>(), "one many many many two");
        assert_eq!(clipped.runs.iter().filter_map(|run| run.href.as_deref()).collect::<Vec<_>>(), ["visible"]);
        assert!(!rt.a11y_json().contains("private value"));
        let first = rt.nodes.iter().position(|n| n.key.as_deref() == Some("flow-first")).unwrap();
        let second = rt.nodes.iter().position(|n| n.key.as_deref() == Some("flow-second")).unwrap();
        let flow_path = rt.nodes[first].path.clone();
        rt.drawn[first] = [10.0, 220.0, 30.0, 15.0];
        rt.drawn[second] = [35.0, 220.0, 30.0, 15.0];
        let mut mirror = A11yMirror::default();
        assert!(rt.web_a11y(&mut mirror).is_some());
        rt.drawn[second][0] = 50.0;
        let update: Value = serde_json::from_str(rt.web_a11y(&mut mirror).unwrap()).unwrap();
        assert!(update.get("items").is_none());
        assert!(update["bounds"].as_array().unwrap().iter().any(|entry|
            entry[0] == flow_path && entry[1] == json!([10.0, 220.0, 70.0, 15.0])));
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
                root = root.child(crate::el().key("button").role("button").label("Go")
                    .size(80.0, 30.0).focusable(true).disabled(s.disabled)
                    .on_press(|s: &mut State| s.presses += 1));
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
        rt.update(|s, _| { s.modal = false; s.gone = true; });
        rt.frame(0.0);
        assert!(!rt.web_a11y_action(&event));
        assert!(!rt.web_a11y_action(&focus));
        assert_eq!(rt.state.presses, 0);
    }
}
