use super::*;
use serde_json::{Value, json};

fn key<S: 'static>(rt: &mut Runtime<S>, key: crate::Key, shift: bool) {
    rt.key(crate::KeyEvent { key, mods: crate::Mods { shift, ..Default::default() } });
    rt.frame(0.0);
}

fn focused<S: 'static>(rt: &Runtime<S>) -> Option<String> {
    rt.elements().into_iter().find(|e| e.focused).and_then(|e| e.key)
}

fn packet<S: 'static>(rt: &Runtime<S>) -> Value {
    let mut mirror = Mirror::default();
    serde_json::from_str(mirror.sync(rt, false).unwrap()).unwrap()
}

fn labelled<'a>(packet: &'a Value, label: &str) -> &'a Value {
    packet["nodes"].as_array().unwrap().iter().find(|n| n["label"] == label).unwrap()
}

fn browser_edit(key: &str, text: &str, kind: &str) -> Value {
    json!({"kind": kind, "key": key, "base": 0, "expected": text,
        "start": text.len(), "end": text.len(), "text": "X", "anchor": text.len() + 1, "focus": text.len() + 1})
}

#[test]
fn an_empty_tree_and_a_single_readonly_field_have_stable_tab_behavior() {
    let mut empty = Runtime::new((), |_: &(), _| crate::col().size(200.0, 100.0), (200.0, 100.0), Env::default());
    empty.frame(0.0);
    key(&mut empty, crate::Key::Tab, false);
    key(&mut empty, crate::Key::Tab, true);
    assert!(empty.focused.is_none());

    struct Single { edit: crate::TextEdit, revision: usize }
    fn view(s: &Single, _: &mut Cx<Single>) -> El {
        crate::col().size(200.0, 100.0).child(
            crate::text_input(&s.edit, crate::TextStyle::new(14.0, 400.0)).key("readonly")
                .label("Read-only").size(180.0, 32.0))
    }
    let mut rt = Runtime::new(Single { edit: crate::TextEdit::new("abc"), revision: 0 }, view,
        (200.0, 100.0), Env::default());
    rt.set_full_keyboard_access(false);
    rt.frame(0.0);
    let tree = packet(&rt);
    let id = labelled(&tree, "Read-only")["id"].clone();
    assert!(rt.web_accessibility_action(&json!({"id": id, "action": "focus"})));
    assert_eq!(focused(&rt).as_deref(), Some("readonly"));
    assert!(rt.web_accessibility_action(&json!({"action": "blur"})));
    assert!(rt.focused.is_none());
    key(&mut rt, crate::Key::Tab, false);
    assert_eq!(focused(&rt).as_deref(), Some("readonly"));
    key(&mut rt, crate::Key::Tab, true);
    assert_eq!(focused(&rt).as_deref(), Some("readonly"));
    let path = rt.focused.clone().unwrap();
    rt.update(|s, _| s.revision += 1);
    rt.frame(0.0);
    assert_eq!(rt.focused.as_deref(), Some(path.as_str()));
    assert!(rt.node(&path).unwrap().input.as_ref().unwrap().edit.focused);
    let tree = packet(&rt);
    let node = labelled(&tree, "Read-only");
    assert_eq!(node["tab"], 0);
    assert_eq!(node["focusable"], true);
    assert_eq!(node["edit"]["readonly"], true);
    assert_eq!(node["edit"]["text"], "abc");
    assert_eq!(tree["focus"], node["id"]);

    assert!(!rt.web_edit(&browser_edit(&path, "abc", "edit")));
    key(&mut rt, crate::Key::Text("x".into()), false);
    rt.paste("p");
    rt.ime(crate::Ime::Commit("i".into()));
    rt.frame(0.0);
    assert_eq!(rt.state.edit.text, "abc");
}

struct Shared { edit: crate::TextEdit, revision: usize }

fn shared_view(s: &Shared, _: &mut Cx<Shared>) -> El {
    crate::col().key("root").size(200.0, 120.0)
        .child(crate::text_input(&s.edit, crate::TextStyle::new(14.0, 400.0))
            .key("readonly").label("Read-only").size(180.0, 32.0))
        .child(crate::text_input(&s.edit, crate::TextStyle::new(14.0, 400.0))
            .key("editable").label("Editable").size(180.0, 32.0).on_edit(|s: &mut Shared| &mut s.edit))
}

#[test]
fn readonly_and_editable_views_of_one_model_keep_distinct_focus() {
    let mut edit = crate::TextEdit::new("abc");
    edit.focused = true;
    let mut rt = Runtime::new(Shared { edit, revision: 0 }, shared_view, (200.0, 120.0), Env::default());
    rt.frame(0.0);
    assert_eq!(focused(&rt).as_deref(), Some("editable"));
    rt.update(|s, _| s.revision += 1);
    rt.frame(0.0);
    assert_eq!(focused(&rt).as_deref(), Some("editable"), "the readonly projection must not steal editable focus");
    key(&mut rt, crate::Key::Tab, false);
    assert_eq!(focused(&rt).as_deref(), Some("readonly"));
    assert!(!rt.state.edit.focused, "readonly focus must not request editable access to the shared model");
    let path = rt.focused.clone().unwrap();
    rt.update(|s, _| s.revision += 1);
    rt.frame(0.0);
    assert_eq!(rt.focused.as_deref(), Some(path.as_str()));
    assert!(rt.node(&path).unwrap().input.as_ref().unwrap().edit.focused);
    key(&mut rt, crate::Key::Text("x".into()), false);
    rt.paste("p"); rt.frame(0.0);
    assert_eq!(rt.state.edit.text, "abc");

    rt.update(|s, _| s.edit.focused = true);
    rt.frame(0.0);
    assert_eq!(focused(&rt).as_deref(), Some("editable"), "an explicit editable request wins over readonly focus");
    rt.update(|s, _| s.edit.set_caret(3));
    key(&mut rt, crate::Key::Text("K".into()), false);
    assert_eq!(rt.state.edit.text, "abcK");
}

struct Gated { edit: crate::TextEdit, disabled: bool }

fn gated_view(s: &Gated, _: &mut Cx<Gated>) -> El {
    crate::col().key("root").size(200.0, 100.0).disabled(s.disabled).child(
        crate::text_input(&s.edit, crate::TextStyle::new(14.0, 400.0)).key("field")
            .label("Field").size(180.0, 32.0).on_edit(|s: &mut Gated| &mut s.edit))
}

#[test]
fn disabling_a_focused_ancestor_revokes_editor_admission_and_clears_composition() {
    let mut edit = crate::TextEdit::new("abc");
    edit.set_caret(3);
    edit.focused = true;
    let mut rt = Runtime::new(Gated { edit, disabled: false }, gated_view, (200.0, 100.0), Env::default());
    rt.frame(0.0);
    let path = rt.focused.clone().unwrap();
    rt.ime(crate::Ime::Preedit("pending".into(), None));
    rt.frame(0.0);
    assert!(rt.state.edit.preedit.is_some());
    rt.update(|s, _| s.disabled = true);
    rt.frame(0.0);
    assert!(rt.focused.is_none());
    assert!(!rt.state.edit.focused && rt.state.edit.preedit.is_none());
    let tree = packet(&rt);
    assert_eq!(labelled(&tree, "Field")["disabled"], true, "disabled fields remain semantically readable");

    // Exercise the operation gate even if an event carries an already-stale focus path.
    rt.focused = Some(path.clone());
    for kind in ["edit", "paste_edit", "preedit"] {
        assert!(!rt.web_edit(&browser_edit(&path, "abc", kind)), "{kind}");
    }
    rt.key(crate::KeyEvent { key: crate::Key::Text("K".into()), mods: Default::default() });
    rt.paste("P");
    rt.ime(crate::Ime::Preedit("again".into(), None));
    rt.ime(crate::Ime::Commit("I".into()));
    rt.frame(0.0);
    assert_eq!(rt.state.edit.text, "abc");
    assert!(rt.state.edit.preedit.is_none());

    rt.update(|s, _| s.disabled = false);
    rt.frame(0.0);
    assert!(rt.focused.is_none(), "reenabling alone does not silently restore editor focus");
    let tree = packet(&rt);
    let id = labelled(&tree, "Field")["id"].clone();
    assert!(rt.web_accessibility_action(&json!({"id": id, "action": "focus"})));
    key(&mut rt, crate::Key::Text("K".into()), false);
    assert_eq!(rt.state.edit.text, "abcK");
}

struct Dialog { edit: crate::TextEdit, open: bool }

fn dialog_view(s: &Dialog, _: &mut Cx<Dialog>) -> El {
    let mut root = crate::col().key("root").size(300.0, 200.0).child(
        crate::el().key("opener").label("Opener").role("button").focusable(true).size(80.0, 30.0)
            .on_press(|s: &mut Dialog| s.open = true));
    if s.open {
        root = root.child(crate::col().key("dialog").absolute(100.0, 0.0).size(180.0, 100.0)
            .modal(true, |s: &mut Dialog| s.open = false)
            .child(crate::text_input(&s.edit, crate::TextStyle::new(14.0, 400.0))
                .key("readonly").label("Dialog text").size(150.0, 32.0))
            .child(crate::el().key("close").role("button").focusable(true).size(70.0, 30.0)));
    }
    root
}

#[test]
fn readonly_field_participates_in_modal_tab_cycle_and_restores_opener() {
    let mut rt = Runtime::new(Dialog { edit: crate::TextEdit::new("unchanged"), open: false }, dialog_view,
        (300.0, 200.0), Env::default());
    rt.frame(0.0);
    key(&mut rt, crate::Key::Tab, false);
    assert_eq!(focused(&rt).as_deref(), Some("opener"));
    key(&mut rt, crate::Key::Text(" ".into()), false);
    assert!(rt.state.open);
    assert_eq!(focused(&rt).as_deref(), Some("readonly"));
    let tree = packet(&rt);
    let node = labelled(&tree, "Dialog text");
    assert_eq!(node["tab"], 0);
    assert_eq!(tree["focus"], node["id"]);
    key(&mut rt, crate::Key::Tab, false);
    assert_eq!(focused(&rt).as_deref(), Some("close"));
    key(&mut rt, crate::Key::Tab, false);
    assert_eq!(focused(&rt).as_deref(), Some("readonly"));
    key(&mut rt, crate::Key::Tab, true);
    assert_eq!(focused(&rt).as_deref(), Some("close"));
    key(&mut rt, crate::Key::Tab, true);
    assert_eq!(focused(&rt).as_deref(), Some("readonly"));
    key(&mut rt, crate::Key::Text("x".into()), false);
    assert_eq!(rt.state.edit.text, "unchanged");
    key(&mut rt, crate::Key::Escape, false);
    assert!(!rt.state.open);
    assert_eq!(focused(&rt).as_deref(), Some("opener"));
    assert_eq!(rt.state.edit.text, "unchanged");
}

struct Virtual {
    rows: Vec<crate::TextEdit>,
    ancestor_disabled: bool,
    row_disabled: bool,
    indexed: bool,
}

fn virtual_row(s: &Virtual, i: usize) -> El {
    crate::col().key(format!("row-{i}")).disabled(i == 0 && s.row_disabled).child(
        crate::text_input(&s.rows[i], crate::TextStyle::new(14.0, 400.0)).key("field").size(200.0, 40.0)
            .on_edit(move |s: &mut Virtual| &mut s.rows[i]))
}

fn virtual_view(s: &Virtual, _: &mut Cx<Virtual>) -> El {
    let rows = crate::col().key("rows").w_full().growth(crate::GrowthSpec::default());
    let rows = if s.indexed {
        rows.virtual_rows(s.rows.len(), 40.0, |_: &Virtual, i| format!("row-{i}").into(), virtual_row)
    } else {
        rows.virtualize(40.0).children((0..s.rows.len()).map(|i| virtual_row(s, i)))
    };
    crate::col().key("root").size(200.0, 200.0).child(
        crate::col().key("viewport").size(200.0, 180.0).disabled(s.ancestor_disabled)
            .scroll(crate::ScrollSpec { stick_bottom: false }).clip_soft(0.0).child(rows))
}

#[test]
fn disabled_virtual_ancestors_and_rows_revoke_offscreen_editor_access() {
    for (indexed, disable_row) in [(false, false), (false, true), (true, false), (true, true)] {
        let mut rows: Vec<_> = (0..100).map(|_| crate::TextEdit::new("abc")).collect();
        rows[0].set_caret(3);
        rows[0].focused = true;
        let mut rt = Runtime::new(Virtual { rows, ancestor_disabled: false, row_disabled: false, indexed }, virtual_view,
            (200.0, 200.0), Env::default());
        rt.frame(0.0);
        let path = rt.focused.clone().unwrap();
        rt.wheel(100.0, 100.0, -2000.0);
        rt.frame(0.0);
        assert!(rt.node(&path).is_none());
        assert_eq!(rt.focused.as_deref(), Some(path.as_str()));
        key(&mut rt, crate::Key::Text("V".into()), false);
        rt.paste("P");
        rt.ime(crate::Ime::Preedit("pending".into(), None));
        rt.frame(0.0);
        assert_eq!(rt.state.rows[0].text, "abcVP");
        assert!(rt.state.rows[0].preedit.is_some());
        rt.update(move |s, _| if disable_row { s.row_disabled = true } else { s.ancestor_disabled = true });
        rt.frame(0.0);
        assert!(rt.focused.is_none());
        assert!(!rt.state.rows[0].focused && rt.state.rows[0].preedit.is_none());
        assert!(!rt.web_edit(&browser_edit(&path, "abcVP", "edit")));
        rt.key(crate::KeyEvent { key: crate::Key::Text("K".into()), mods: Default::default() });
        rt.paste("P");
        rt.ime(crate::Ime::Commit("I".into()));
        rt.frame(0.0);
        assert_eq!(rt.state.rows[0].text, "abcVP");

        rt.update(|s, _| { s.row_disabled = false; s.ancestor_disabled = false; });
        rt.frame(0.0);
        assert!(rt.focused.is_none());
        // Index-built rows discover an offscreen field only while it remains focused; remount
        // it before making a new focus request after the disabling transition.
        if indexed { rt.wheel(100.0, 100.0, 2000.0); rt.frame(0.0); }
        rt.update(|s, _| s.rows[0].focused = true);
        rt.frame(0.0);
        assert_eq!(rt.focused.as_deref(), Some(path.as_str()));
        key(&mut rt, crate::Key::Text("R".into()), false);
        assert_eq!(rt.state.rows[0].text, "abcVPR");
    }
}
