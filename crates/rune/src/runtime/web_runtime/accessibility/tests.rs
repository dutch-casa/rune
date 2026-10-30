use super::*;
use serde_json::{Value, json};

struct State {
    draft: crate::TextEdit,
    secret: crate::TextEdit,
    present: bool,
    inert: bool,
    modal: bool,
    clicks: usize,
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    let button = || crate::el().key("send").role("button").label("Send").focusable(true).size(90.0, 36.0)
        .on_press(|s: &mut State| s.clicks += 1);
    let mut root = crate::col().key("root").role("main").w_full().h_full()
        .child(crate::text("Hello 🦀").key("message"))
        .child(crate::text_input(&s.draft, crate::tok::type_::BODY).key("draft").label("Message").size(200.0, 48.0)
            .on_edit(|s: &mut State| &mut s.draft))
        .child(crate::text_input(&s.secret, crate::tok::type_::BODY).key("secret").label("Password").size(200.0, 48.0)
            .on_edit(|s: &mut State| &mut s.secret))
        .child(crate::text_input(&s.draft, crate::tok::type_::BODY).key("readonly").label("Read-only draft").size(200.0, 48.0))
        .child(crate::el().key("mixed").role("checkbox").label("Include descendants").indeterminate(true).size(32.0, 32.0));
    if s.present {
        let group = crate::col();
        root = root.child((if s.inert { group.inert() } else { group }).child(button()));
    }
    if s.modal {
        root = root.child(crate::col().key("dialog").role("dialog").label("Confirm").size(200.0, 100.0)
            .modal(true, |s: &mut State| s.modal = false)
            .child(crate::el().key("close").role("button").label("Close").focusable(true).size(60.0, 36.0)
                .on_press(|s: &mut State| s.modal = false)));
    }
    root
}

fn runtime() -> Runtime<State> {
    let mut rt = Runtime::new(State { draft: crate::TextEdit::new("a🦀é".repeat(900)),
        secret: crate::TextEdit::new("must-not-appear-in-DOM").secure(), present: true, inert: false, modal: false, clicks: 0 },
        view, (300.0, 400.0), Env::default());
    rt.frame(0.0);
    rt
}

fn packet(mirror: &mut Mirror, rt: &Runtime<State>) -> Value {
    serde_json::from_str(mirror.sync(rt, false).expect("changed semantic state")).unwrap()
}

fn node<'a>(packet: &'a Value, name: &str) -> &'a Value {
    packet["nodes"].as_array().unwrap().iter().find(|n| n["label"] == name).unwrap()
}

#[test]
fn exposes_content_and_state_without_secret_or_unbounded_editor_copies() {
    let rt = runtime();
    let mut mirror = Mirror::default();
    let tree = packet(&mut mirror, &rt);
    assert!(tree["nodes"].as_array().unwrap().iter().any(|n| n["text"] == "Hello 🦀"));
    assert_eq!(node(&tree, "Send")["role"], "button");
    assert_eq!(node(&tree, "Include descendants")["checked"], "mixed");
    let draft = &node(&tree, "Message")["edit"];
    assert!(draft["text"].as_str().unwrap().encode_utf16().count() <= 1024);
    assert_eq!(draft["readonly"], false);
    assert!(serde_json::to_string(node(&tree, "Message")).unwrap().len() < rt.state.draft.text.len(),
        "an editable node must not also serialize its full document");
    let readonly = &node(&tree, "Read-only draft")["edit"];
    assert_eq!(readonly["readonly"], true);
    assert_eq!(readonly["text"], "a🦀é".repeat(900));
    let secret = &node(&tree, "Password")["edit"];
    assert_eq!(secret["secure"], true);
    assert_eq!(secret["text"], "");
    assert_eq!(secret["anchor"], 0);
    assert!(!serde_json::to_string(&tree).unwrap().contains("must-not-appear-in-DOM"));
    assert!(mirror.sync(&rt, false).is_none(), "an unchanged frame delivers no packet");
}

#[test]
fn accessibility_focus_and_activation_reach_the_retained_operation() {
    let mut rt = runtime();
    let mut mirror = Mirror::default();
    let tree = packet(&mut mirror, &rt);
    let id = node(&tree, "Send")["id"].clone();
    assert!(rt.web_accessibility_action(&json!({"id":id,"action":"focus"})));
    rt.frame(0.0);
    let focused = packet(&mut mirror, &rt);
    assert_eq!(focused["focus"], id);
    assert!(rt.web_accessibility_action(&json!({"id":id,"action":"activate"})));
    rt.frame(0.0);
    assert_eq!(rt.state.clicks, 1);
}

#[test]
fn removed_inert_and_modal_background_targets_cannot_be_invoked() {
    for condition in ["removed", "inert", "modal"] {
        let mut rt = runtime();
        let mut mirror = Mirror::default();
        let tree = packet(&mut mirror, &rt);
        let id = node(&tree, "Send")["id"].clone();
        rt.update(|s, _| match condition { "removed" => s.present = false, "inert" => s.inert = true, _ => s.modal = true });
        rt.frame(0.0);
        assert!(!rt.web_accessibility_action(&json!({"id":id,"action":"activate"})), "{condition}");
        assert!(!rt.web_accessibility_action(&json!({"id":id,"action":"focus"})), "{condition}");
        rt.frame(0.0);
        assert_eq!(rt.state.clicks, 0, "{condition}");
        let tree = packet(&mut mirror, &rt);
        assert!(!tree["nodes"].as_array().unwrap().iter().any(|n| n["label"] == "Send"));
        if condition == "modal" {
            assert_eq!(tree["modal"], true);
            assert_eq!(node(&tree, "Confirm")["modal"], true);
            assert_eq!(node(&tree, "Close")["role"], "button");
        }
    }
}

#[test]
fn rejects_malformed_identity_and_unknown_actions() {
    let mut rt = runtime();
    let mut mirror = Mirror::default();
    let tree = packet(&mut mirror, &rt);
    let id = node(&tree, "Send")["id"].clone();
    for event in [json!({"id":"18446744073709551616","action":"activate"}),
        json!({"id":1,"action":"activate"}), json!({"id":id,"action":"execute"})] {
        assert!(!rt.web_accessibility_action(&event));
    }
    rt.frame(0.0);
    assert_eq!(rt.state.clicks, 0);
}

#[test]
fn key_release_handlers_survive_semantic_dom_focus() {
    let mut rt = Runtime::new(0_usize, |_: &usize, _| crate::col().w_full().h_full()
        .on_key_up(|released: &mut usize, event, _| {
            if event.key != crate::Key::Enter { return false; }
            *released += 1;
            true
        }), (300.0, 400.0), Env::default());
    rt.frame(0.0);
    for code in ["", "Enter"] {
        rt.web_key(&json!({"key":"Enter","code":code,"up":true}));
        rt.frame(0.0);
    }
    assert_eq!(rt.state, 2);
}

#[test]
fn disabled_content_remains_readable_but_cannot_be_activated_or_focused() {
    let mut rt = Runtime::new(0_usize, |_: &usize, _| crate::col().disabled(true)
        .child(crate::el().role("button").label("Unavailable").focusable(true).size(80.0, 32.0)
            .on_press(|s: &mut usize| *s += 1))
        .child(crate::text("Still readable")), (300.0, 200.0), Env::default());
    rt.frame(0.0);
    let tree: Value = serde_json::from_str(Mirror::default().sync(&rt, false).unwrap()).unwrap();
    let id = node(&tree, "Unavailable")["id"].clone();
    assert!(tree["nodes"].as_array().unwrap().iter().any(|n| n["text"] == "Still readable"));
    assert!(!rt.web_accessibility_action(&json!({"id":id,"action":"focus"})));
    assert!(!rt.web_accessibility_action(&json!({"id":id,"action":"activate"})));
    rt.frame(0.0);
    assert_eq!(rt.state, 0);
}

#[test]
fn inline_link_focus_activation_and_stale_admission_use_existing_link_owner() {
    let mut rt = Runtime::new((0_usize, true), |s: &(usize, bool), _| {
        if !s.1 { return crate::col(); }
        crate::text("Read the guide").key("prose")
            .link(crate::Link::new(9..14, "guide"))
            .on_link(|s: &mut (usize, bool), id, _| { if id == "guide" { s.0 += 1; } })
    }, (300.0, 200.0), Env::default());
    rt.frame(0.0);
    let mut mirror = Mirror::default();
    let tree: Value = serde_json::from_str(mirror.sync(&rt, false).unwrap()).unwrap();
    let id = node(&tree, "guide")["id"].clone();
    assert!(rt.web_accessibility_action(&json!({"id":id,"action":"focus"})));
    assert!(rt.web_accessibility_action(&json!({"id":id,"action":"activate"})));
    rt.frame(0.0);
    assert_eq!(rt.state.0, 1);
    let tree: Value = serde_json::from_str(mirror.sync(&rt, false).unwrap()).unwrap();
    assert_eq!(tree["focus"], id);
    rt.update(|s, _| s.1 = false);
    rt.frame(0.0);
    assert!(!rt.web_accessibility_action(&json!({"id":id,"action":"activate"})));
    rt.frame(0.0);
    assert_eq!(rt.state.0, 1);
}

#[test]
fn live_text_reaches_the_browser_without_a_view_commit() {
    let mut rt = Runtime::new(crate::LiveText::new("Before"), |s, _| {
        crate::text("Before").size(100.0, 30.0).live_text(s)
    }, (300.0, 200.0), Env::default());
    rt.frame(0.0);
    let mut mirror = Mirror::default();
    let _: Value = serde_json::from_str(mirror.sync(&rt, false).unwrap()).unwrap();
    let commits = rt.commits;
    rt.state.set("After");
    rt.frame(0.016);
    assert_eq!(rt.commits, commits);
    let tree: Value = serde_json::from_str(mirror.sync(&rt, false).unwrap()).unwrap();
    assert!(tree["nodes"].as_array().unwrap().iter().any(|n| n["text"] == "After"));
}

#[test]
fn readonly_projection_cannot_request_its_models_editable_focus() {
    let mut rt = runtime();
    let mut mirror = Mirror::default();
    let tree = packet(&mut mirror, &rt);
    let id = node(&tree, "Message")["id"].clone();
    assert!(rt.web_accessibility_action(&json!({"id": id, "action": "focus"})));
    rt.frame(0.0);
    let key = rt.focused.clone().unwrap();
    let mut context = super::super::Mirror::default();
    assert!(rt.web_context(&mut context));
    assert!(rt.web_edit(&json!({"kind": "edit", "key": key, "base": context.base,
        "expected": context.text, "start": 0, "end": 0, "text": "x", "anchor": 1, "focus": 1})));
    rt.frame(0.0);
    assert_eq!(rt.focused.as_deref(), Some(key.as_str()));
    assert_eq!(packet(&mut mirror, &rt)["focus"], id);
}

#[test]
fn inline_prose_is_exposed_once_in_source_order_with_stable_link_identity() {
    for content in ["", "é🦀!"] {
        let mut rt = Runtime::new(content, |s, _| {
            crate::text(*s).link(crate::Link::new(2..6, "crab")).link(crate::Link::new(0..2, "accent"))
        }, (300.0, 200.0), Env::default());
        rt.frame(0.0);
        let tree: Value = serde_json::from_str(Mirror::default().sync(&rt, false).unwrap()).unwrap();
        let nodes = tree["nodes"].as_array().unwrap();
        let text: String = nodes.iter().flat_map(|n| [n["text"].as_str().unwrap_or(""), n["after"].as_str().unwrap_or("")]).collect();
        assert_eq!(text, content);
        if !content.is_empty() {
            let links: Vec<_> = nodes.iter().filter(|n| n["role"] == "link").collect();
            assert_eq!(links[0]["text"], "é");
            assert_eq!(links[1]["text"], "🦀");
            assert!(links[0]["id"].as_str().unwrap().ends_with("#link-1"));
            assert!(links[1]["id"].as_str().unwrap().ends_with("#link-0"));
            assert_eq!(links[1]["after"], "!");
        }
    }
}

#[test]
fn empty_shell_names_do_not_mask_editor_names_or_browser_fallbacks() {
    let mut rt = Runtime::new(crate::TextEdit::new("value"), |s, _| crate::col()
        .child(crate::col().role("textbox").fallback_label("")
            .child(crate::text_input(s, crate::tok::type_::BODY).key("unnamed")))
        .child(crate::col().role("textbox").label("Field name")
            .child(crate::text_input(s, crate::tok::type_::BODY).key("field").fallback_label("")))
        .child(crate::col().role("textbox").label("Group name")
            .child(crate::text_input(s, crate::tok::type_::BODY).key("explicit").label("Control name"))),
        (300.0, 200.0), Env::default());
    rt.frame(0.0);
    let tree: Value = serde_json::from_str(Mirror::default().sync(&rt, false).unwrap()).unwrap();
    let editors: Vec<_> = tree["nodes"].as_array().unwrap().iter().filter(|n| n["edit"].is_object()).collect();
    assert_eq!(editors[0]["label"], Value::Null);
    assert_eq!(editors[1]["label"], "Field name");
    assert_eq!(editors[2]["label"], "Control name");
}

#[test]
fn clearing_indeterminate_restores_the_binary_state_in_browser_controls() {
    let mut rt = Runtime::new((true, true), |s: &(bool, bool), _| crate::col()
        .child(crate::el().role("checkbox").label("First").checked(s.0).indeterminate(s.1))
        .child(crate::el().role("checkbox").label("Second").indeterminate(s.1).checked(s.0)),
        (300.0, 200.0), Env::default());
    let mut mirror = Mirror::default();
    for (checked, indeterminate, expected) in [(true, true, "mixed"), (true, false, "true"), (false, false, "false"), (false, true, "mixed")] {
        rt.update(|s, _| *s = (checked, indeterminate));
        rt.frame(0.0);
        let tree: Value = serde_json::from_str(mirror.sync(&rt, false).unwrap()).unwrap();
        assert_eq!(node(&tree, "First")["checked"], expected);
        assert_eq!(node(&tree, "Second")["checked"], expected);
    }
}
