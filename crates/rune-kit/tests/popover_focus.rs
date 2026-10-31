use rune::{Cx, El, Env, Key, KeyEvent, Mods, Runtime};
use rune_kit::{Button, Kit, Popover, root};

#[derive(Default)]
struct State { open: bool }

fn view(s: &State, _: &mut Cx<State>) -> El {
    let k = Kit::neutral(true);
    let mut layer = Vec::new();
    let trigger = Button::new("Open").on_press(|s: &mut State| s.open = true).build(&k).key("open");
    let body = Popover::new("details").trigger(trigger).open(s.open)
        .on_close(|s: &mut State| s.open = false)
        .child(Button::new("Close").on_press(|s: &mut State| s.open = false).build(&k).key("close"))
        .build(&k, &mut layer);
    root(body, layer)
}

#[test]
fn popover_close_restores_its_trigger_before_exit_finishes() {
    let mut rt = Runtime::new(State::default(), view, (400.0, 400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    for key in [Key::Tab, Key::Text(" ".into()), Key::Tab] {
        rt.key(KeyEvent { key, mods: Default::default() }); rt.frame(0.0);
    }
    assert_eq!(rt.elements().into_iter().find(|e| e.focused).unwrap().key.as_deref(), Some("close"));
    rt.key(KeyEvent { key: Key::Text(" ".into()), mods: Default::default() }); rt.frame(0.0);
    assert!(!rt.state.open);
    assert_eq!(rt.elements().into_iter().find(|e| e.focused).unwrap().key.as_deref(), Some("open"));
    for _ in 0..60 { rt.frame(1.0 / 60.0); }
    assert_eq!(rt.elements().into_iter().find(|e| e.focused).unwrap().key.as_deref(), Some("open"));
}

#[test]
fn escape_dismisses_an_open_popover_without_trapping_focus() {
    #[derive(Default)]
    struct S { open: bool }
    fn view(s: &S, _: &mut Cx<S>) -> El {
        let k = Kit::neutral(true);
        let mut layer = Vec::new();
        let trigger = Button::new("Open").on_press(|s: &mut S| s.open = true).build(&k).key("open");
        let pop = Popover::new("details").trigger(trigger).open(s.open)
            .on_close(|s: &mut S| s.open = false)
            .child(Button::new("Inside").on_press(|_: &mut S| {}).build(&k).key("inside"))
            .build(&k, &mut layer);
        let outside = Button::new("Outside").on_press(|_: &mut S| {}).build(&k).key("outside");
        root(rune::col().child(pop).child(outside), layer)
    }
    let mut rt = Runtime::new(S { open: true }, view, (400.0, 400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let focused = |rt: &Runtime<S>| rt.elements().into_iter().find(|e| e.focused).and_then(|e| e.key);
    rt.key(KeyEvent { key: Key::Tab, mods: Default::default() });
    rt.frame(0.0);
    // The trigger stays a tab stop: the popover does not trap focus.
    let mut saw_outside = false;
    let mut saw_inside = false;
    for _ in 0..6 {
        let key = focused(&rt);
        saw_outside |= key.as_deref() == Some("outside");
        saw_inside |= key.as_deref() == Some("inside");
        if saw_inside { break; }
        rt.key(KeyEvent { key: Key::Tab, mods: Default::default() });
        rt.frame(0.0);
    }
    assert!(saw_inside, "Tab reaches the popover content");
    rt.key(KeyEvent { key: Key::Escape, mods: Default::default() });
    rt.frame(0.0);
    assert!(!rt.state.open, "Escape dismisses");
    assert_eq!(focused(&rt).as_deref(), Some("open"));
    // Reopen and confirm Tab can leave the content for the page.
    rt.key(KeyEvent { key: Key::Text(" ".into()), mods: Default::default() });
    rt.frame(0.0);
    assert!(rt.state.open);
    for _ in 0..8 {
        rt.key(KeyEvent { key: Key::Tab, mods: Default::default() });
        rt.frame(0.0);
        if focused(&rt).as_deref() == Some("outside") { saw_outside = true; break; }
    }
    assert!(saw_outside, "a non-modal popover does not trap Tab");
}

#[test]
fn a_modified_escape_leaves_an_open_popover_alone() {
    let mut rt = Runtime::new(State { open: true }, view, (400.0, 400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.key(KeyEvent { key: Key::Tab, mods: Mods::default() });
    rt.frame(0.0);
    for mods in [
        Mods { shift: true, ..Mods::default() },
        Mods { ctrl: true, ..Mods::default() },
        Mods { alt: true, ..Mods::default() },
        Mods { logo: true, ..Mods::default() },
    ] {
        rt.key(KeyEvent { key: Key::Escape, mods });
        rt.frame(0.0);
        assert!(rt.state.open, "modified Escape must reach the app: {mods:?}");
    }
    rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
    rt.frame(0.0);
    assert!(!rt.state.open, "unmodified Escape still dismisses");
}

#[test]
fn a_popover_trigger_reports_whether_its_popup_is_open() {
    let mut rt = Runtime::new(State { open: false }, view, (400.0, 400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let trigger = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("open")).unwrap();
    assert_eq!(trigger.expanded, Some(false));
    assert!(trigger.controls.is_none());
    rt.update(|s, _| s.open = true);
    rt.frame(0.0);
    let trigger = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("open")).unwrap();
    let content = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("details-content")).unwrap();
    assert_ne!(content.role, "dialog");
    assert_eq!(content.role, "group");
    assert_eq!(trigger.expanded, Some(true));
    assert_eq!(trigger.controls.as_deref(), Some(content.path.as_str()));
}
