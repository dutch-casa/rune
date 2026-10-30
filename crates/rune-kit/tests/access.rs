//! Keyboard and state defaults the kit owns: disclosure, checked menu items, toast
//! announcement, and progress value.

use rune::{Cx, El, Env, Key, KeyEvent, Mods, Runtime, ValueRange};
use rune_kit::{
    Accordion, AccordionItem, Collapsible, Kit, Menu, MenuEntry, MenuItem, Progress, ProgressCircle, Spinner, Toast, root,
};

fn press<S>(rt: &mut Runtime<S>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(0.0);
}

fn focused<S>(rt: &Runtime<S>) -> Option<String> {
    rt.elements().into_iter().find(|e| e.focused).and_then(|e| e.key)
}

#[test]
fn accordion_and_collapsible_triggers_are_focusable_disclosures() {
    struct S {
        accordion: bool,
        collapsible: bool,
    }
    fn view(s: &S, _: &mut Cx<S>) -> El {
        let k = Kit::neutral(true);
        rune::col()
            .child(
                Accordion::new()
                    .item(
                        AccordionItem::new("Details")
                            .expanded(s.accordion)
                            .on_toggle(|s: &mut S| s.accordion = !s.accordion)
                            .child(rune::text("Body")),
                    )
                    .build(&k),
            )
            .child(
                Collapsible::new("More", s.collapsible)
                    .on_toggle(|s: &mut S| s.collapsible = !s.collapsible)
                    .child(rune::text("Rest"))
                    .build(&k),
            )
    }
    let mut rt = Runtime::new(S { accordion: false, collapsible: true }, view, (320.0, 240.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let trigger = |rt: &Runtime<S>, title: &str| {
        rt.elements().into_iter().find(|e| e.role == "button" && e.label.as_deref() == Some(title)).unwrap()
    };
    assert!(trigger(&rt, "Details").focusable && trigger(&rt, "Details").expanded == Some(false));
    assert!(trigger(&rt, "More").focusable && trigger(&rt, "More").expanded == Some(true));
    press(&mut rt, Key::Tab);
    assert_eq!(focused(&rt).as_deref(), Some("trigger"));
    press(&mut rt, Key::Text(" ".into()));
    assert!(rt.state.accordion);
    rt.frame(0.0);
    assert_eq!(trigger(&rt, "Details").expanded, Some(true));
}

#[test]
fn a_checked_menu_item_is_a_checkbox_and_a_submenu_reports_open() {
    fn view(_: &(), _: &mut Cx<()>) -> El {
        let k = Kit::neutral(true);
        let mut layer = Vec::new();
        let trigger = Menu::new("file")
            .open(true)
            .item(MenuItem::new("Reload").checked(true))
            .item(MenuItem::new("Quit"))
            .item(MenuItem::new("Share").submenu(vec![MenuEntry::item(MenuItem::new("Copy"))], true))
            .build(&k, &mut layer);
        root(trigger, layer)
    }
    let mut rt = Runtime::new((), view, (400.0, 300.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let item = |label: &str| rt.elements().into_iter().find(|e| e.label.as_deref() == Some(label)).unwrap();
    let reload = item("Reload");
    assert_eq!((reload.role, reload.checked), ("menuitemcheckbox", Some(true)));
    assert_eq!(item("Quit").role, "menuitem");
    assert_eq!(item("Share").expanded, Some(true));
}

#[test]
fn a_toast_announces_its_description_and_its_close_is_a_key() {
    struct S { open: bool }
    fn view(s: &S, _: &mut Cx<S>) -> El {
        let k = Kit::neutral(true);
        if !s.open {
            return rune::el();
        }
        Toast::new("saved", "Saved").description("Draft stored").on_close(|s: &mut S| s.open = false).build(&k)
    }
    let mut rt = Runtime::new(S { open: true }, view, (400.0, 200.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let status = rt.elements().into_iter().find(|e| e.role == "status").unwrap();
    assert_eq!(status.label.as_deref(), Some("Saved. Draft stored"));
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some("Draft stored")));
    press(&mut rt, Key::Tab);
    assert_eq!(focused(&rt).as_deref(), Some("close"));
    press(&mut rt, Key::Text(" ".into()));
    assert!(!rt.state.open);
}

#[test]
fn progress_exposes_its_clamped_percentage_and_spinner_keeps_its_name() {
    fn view(_: &(), _: &mut Cx<()>) -> El {
        let k = Kit::neutral(true);
        rune::col()
            .child(Progress::new(Some(0.5)).label("Upload").show_value().build(&k).key("bar"))
            .child(Progress::new(None).label("Working").build(&k).key("wait"))
            .child(Progress::new(Some(1.4)).build(&k).key("clamped"))
            .child(ProgressCircle::new(0.25).build(&k).key("circle"))
            .child(Spinner::new().build(&k).key("spin"))
    }
    let mut rt = Runtime::new((), view, (240.0, 200.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let at = |key: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).unwrap();
    assert_eq!(at("bar").range, Some(ValueRange { value: 50.0, min: 0.0, max: 100.0 }));
    assert_eq!(at("bar").label.as_deref(), Some("Upload"));
    assert!(at("wait").range.is_none());
    assert_eq!(at("clamped").range, Some(ValueRange { value: 100.0, min: 0.0, max: 100.0 }));
    assert_eq!(at("circle").range, Some(ValueRange { value: 25.0, min: 0.0, max: 100.0 }));
    assert_eq!(at("circle").label.as_deref(), Some("Progress"));
    assert_eq!(at("spin").label.as_deref(), Some("Loading"));
    assert!(at("spin").range.is_none());
}
