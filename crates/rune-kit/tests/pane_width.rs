use rune::{Cx, El, ElementInfo, Env, Key, KeyEvent, Mods, Runtime, text};
use rune_kit::{Kit, shell::{PaneState, Sidebar, SidebarRow, SidebarSection, SplitView}};

struct State { sidebar: PaneState, selected: bool }

fn view(s: &State, _: &mut Cx<State>) -> El {
    let k = Kit::neutral(true);
    let rows = Sidebar::new().section(SidebarSection::new("Today").row(
        SidebarRow::new("Alpha").selected(s.selected).on_select(|s: &mut State, _| s.selected = true).build(&k).key("row1"),
    )).build(&k);
    SplitView::new(text("Main")).sidebar(&s.sidebar, |s: &mut State| &mut s.sidebar, rows).build(&k)
}

fn runtime() -> Runtime<State> {
    let mut rt = Runtime::new(State { sidebar: PaneState::new(240.0), selected: true }, view, (900.0, 600.0), Env::new(true, 1.0));
    settle(&mut rt);
    rt
}

fn settle(rt: &mut Runtime<State>) { for _ in 0..120 { rt.frame(1.0 / 60.0); } }
fn press(rt: &mut Runtime<State>, key: Key) { rt.key(KeyEvent { key, mods: Mods::default() }); rt.frame(1.0 / 60.0); }
fn find(rt: &Runtime<State>, key: &str) -> ElementInfo { rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).expect("pane control") }
fn choose_narrow(rt: &mut Runtime<State>) {
    settle(rt);
    let item = rt.elements().into_iter().find(|e| e.role == "menuitem" && e.label.as_deref() == Some("Narrow")).expect("step menu item");
    let (x, y) = (item.rect[0] + item.rect[2] * 0.5, item.rect[1] + item.rect[3] * 0.5);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.0);
}

#[test]
fn grip_keys_resize_and_obey_the_panes_limits() {
    let mut rt = runtime();
    let mut reached = false;
    for _ in 0..8 {
        press(&mut rt, Key::Tab);
        if find(&rt, "sidebar-grip").focused { reached = true; break; }
    }
    assert!(reached, "Tab reaches the resize grip");
    let before = rt.state.sidebar.width();
    press(&mut rt, Key::Right);
    assert_eq!(rt.state.sidebar.width(), before + 8.0);
    press(&mut rt, Key::Home);
    assert_eq!(rt.state.sidebar.width(), rt.state.sidebar.min_width());
    press(&mut rt, Key::End);
    assert_eq!(rt.state.sidebar.width(), rt.state.sidebar.max_width());
}

#[test]
fn secondary_click_offers_non_drag_resizing() {
    let mut rt = runtime();
    let before = rt.state.sidebar.width();
    let (x, y) = rt.find("sidebar-grip").unwrap();
    rt.secondary_down(x, y);
    rt.secondary_up(x, y);
    choose_narrow(&mut rt);
    assert_eq!(rt.state.sidebar.width(), before - 8.0);
    assert!(rt.elements().iter().all(|e| e.role != "menu" || e.exiting));
}

#[test]
fn primary_width_control_does_not_interfere_with_dragging_or_tab_order() {
    let mut rt = runtime();
    let before = rt.state.sidebar.width();
    let grip = find(&rt, "sidebar-grip");
    let width = find(&rt, "sidebar-width");
    press(&mut rt, Key::Tab);
    assert!(find(&rt, "row1").focused, "the extra pointer control does not take the row's Tab stop");
    assert!(width.rect[2] >= 23.5 && width.rect[3] >= 23.5);
    assert!(width.rect[0] + width.rect[2] <= grip.rect[0] + 0.5, "the control is beside the grip");
    let (x, y) = (grip.rect[0] + grip.rect[2] * 0.5, grip.rect[1] + grip.rect[3] * 0.5);
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_move(x + 40.0, y);
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.sidebar.width(), before + 40.0);
    rt.pointer_up(x + 40.0, y);
    rt.frame(0.0);
    assert!(rt.elements().iter().all(|e| e.role != "menu"));
    let (x, y) = rt.find("sidebar-width").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    choose_narrow(&mut rt);
    assert_eq!(rt.state.sidebar.width(), before + 32.0);
    assert!(rt.elements().iter().all(|e| e.role != "menu" || e.exiting));
}
