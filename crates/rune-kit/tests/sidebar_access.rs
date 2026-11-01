use std::borrow::Cow;

use rune::{Cx, El, Env, Key, KeyEvent, Mods, Runtime, col};
use rune_kit::Kit;
use rune_kit::shell::{Sidebar, SidebarHeader, SidebarRow, SidebarSection};

#[test]
fn a_section_header_includes_its_count_in_the_name() {
    fn view(_: &(), _: &mut Cx<()>) -> El {
        let k = Kit::neutral(true);
        Sidebar::new().section(SidebarSection::new("Today").count(4).row(SidebarRow::new("Alpha").build(&k))).build(&k)
    }
    let mut rt = Runtime::new((), view, (240.0, 200.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let header = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("section-header")).unwrap();
    assert_eq!(header.label.as_deref(), Some("Today, 4"));
}

#[test]
fn delete_on_a_focused_row_runs_its_close() {
    struct C { closed: bool }
    fn view(_: &C, _: &mut Cx<C>) -> El {
        let k = Kit::neutral(true);
        Sidebar::new().section(
            SidebarSection::new("Today").row(
                SidebarRow::new("Alpha").selected(true).on_close(|s: &mut C, _| s.closed = true).build(&k).key("row"),
            ),
        ).build(&k)
    }
    let mut rt = Runtime::new(C { closed: false }, view, (240.0, 200.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.key(KeyEvent { key: Key::Tab, mods: Mods::default() });
    rt.frame(1.0 / 60.0);
    let close = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("close")).unwrap();
    assert!(!close.focusable);
    let row = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("row")).unwrap();
    assert_eq!(row.key_shortcuts.as_deref(), Some("Delete"));
    rt.key(KeyEvent { key: Key::Delete, mods: Mods::default() });
    rt.frame(1.0 / 60.0);
    assert!(rt.state.closed);
}

#[test]
fn a_virtual_header_toggle_keeps_focus_independent_of_the_selected_row() {
    fn view(_: &bool, _: &mut Cx<bool>) -> El {
        let k = Kit::neutral(true);
        let rows = k.clone();
        col().w_full().h_full().child(
            Sidebar::new().virtual_rows(2, |_: &bool, i| Cow::Owned(if i == 0 { "head".into() } else { "row".into() }), move |collapsed, i| {
                if i == 0 {
                    SidebarHeader::new("Today").collapsed(*collapsed).on_toggle(|c: &mut bool, _| *c = !*c).build(&rows)
                } else {
                    SidebarRow::new("Alpha").selected(!*collapsed).on_select(|_: &mut bool, _| {}).build(&rows).key("alpha")
                }
            }).build(&k),
        )
    }
    let mut rt = Runtime::new(false, view, (240.0, 300.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(0.0);
    let header = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("head")).expect("virtual header").path;
    rt.key(KeyEvent { key: Key::Tab, mods: Mods::default() });
    rt.frame(1.0 / 60.0);
    assert!(rt.elements().iter().any(|e| e.focused && e.path.starts_with(&header)), "Tab reaches the standalone header");
    rt.key(KeyEvent { key: Key::Left, mods: Mods { shift: true, ..Mods::default() } });
    rt.frame(0.0);
    assert!(!rt.state, "a modified Left does not collapse");
    rt.key(KeyEvent { key: Key::Left, mods: Mods::default() });
    rt.frame(0.0);
    assert!(rt.state, "Left on the expanded stop collapses it");
}
