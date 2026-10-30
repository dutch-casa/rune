//! Browser input regression fixture, selected only by ?input-proof.
use rune::{App, AppHandle, AppSpec, Cx, El, Key, Link, ScrollSpec, TextEdit, col, el, rich, row, text, text_input, tok};
use rune_kit::{Button, Checkbox, Field, Kit, Select, SelectAppearance, TextInput, root};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = "
export function enabled() { return new URLSearchParams(location.search).has('input-proof'); }
export function disputed() { return new URLSearchParams(location.search).has('disputed-review'); }
export function reportActions(first, second, launch, before, after) {
    window.__disputedActions = { first, second, launch, before, after };
}
export function reportScroll(offset) { window.__disputedScroll = offset; }
export function report(plain, display, single, secretLength, submits, shortcuts, visible) {
    window.__inputProof = { plain, display, single, secretLength, submits, shortcuts, visible,
        polls: (window.__inputProof?.polls ?? 0) + 1 };
}
")]
extern "C" {
    pub fn enabled() -> bool;
    fn disputed() -> bool;
    fn reportActions(first: u32, second: u32, launch: u32, before: u32, after: u32);
    fn reportScroll(offset: f32);
    fn report(plain: &str, display: &str, single: &str, secret_length: usize, submits: u32, shortcuts: u32, visible: bool);
}

struct State {
    plain: TextEdit,
    single: TextEdit,
    secret: TextEdit,
    captioned: TextEdit,
    explicit: TextEdit,
    range_start: TextEdit,
    range_end: TextEdit,
    fruit: String,
    select_open: bool,
    help: bool,
    checked: bool,
    submits: u32,
    shortcuts: u32,
    first: u32,
    second: u32,
    launch: u32,
    before: u32,
    after: u32,
    handle: Option<AppHandle<State>>,
}

impl Default for State {
    fn default() -> Self {
        Self { plain: TextEdit::default(), single: TextEdit::default(), secret: TextEdit::default().secure(),
            captioned: TextEdit::default(), fruit: "Banana".into(), select_open: false, help: true, checked: false,
            explicit: TextEdit::default(), range_start: TextEdit::default(), range_end: TextEdit::default(),
            submits: 0, shortcuts: 0, first: 0, second: 0, launch: 0, before: 0, after: 0, handle: None }
    }
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    if disputed() { return disputed_view(s); }
    let field = |edit: &TextEdit| text_input(edit, tok::type_::BODY).w_full().h(48.0);
    let k = Kit::neutral(true);
    let mut layers = Vec::new();
    let select = Select::new("fruit", &["Apple", "Banana", "Cherry"]).current(Some(&s.fruit))
        .appearance(SelectAppearance::Field).width(120.0)
        .open(s.select_open).on_toggle(|s: &mut State| s.select_open = !s.select_open)
        .on_close(|s: &mut State| s.select_open = false)
        .on_choose(|s: &mut State, value, _| { s.fruit = value.into(); s.select_open = false; })
        .build(&k, &mut layers);
    let button = el().key("button").focusable(true).w_full().h(48.0).child(text("Button"))
        .on_press(|s: &mut State| { s.submits += 1; s.help = !s.help; });
    let button = if s.help { button.help("Button help") } else { button };
    let captioned = TextInput::new(&s.captioned).on_edit(|s: &mut State| &mut s.captioned).build(&k).key("captioned");
    let captioned = if s.help { Field::new().label("Captioned editor") } else { Field::new() }.control(captioned).build(&k);
    let body = col().w_full().h_full().padding(16.0).gap(8.0)
        .child(field(&s.plain).key("plain").on_edit(|s: &mut State| &mut s.plain)
            .on_submit(|s: &mut State, _| s.submits += 1))
        .child(field(&s.single).key("single").single_line().on_edit(|s: &mut State| &mut s.single)
            .on_submit(|s: &mut State, _| s.submits += 1))
        .child(field(&s.secret).key("secret").single_line().on_edit(|s: &mut State| &mut s.secret)
            .on_submit(|s: &mut State, _| s.submits += 1))
        .child(button)
        .child(Field::new().label("Fruit").control(select).build(&k))
        .child(captioned)
        .child(Checkbox::new(s.checked).label("Visible checkbox")
            .on_toggle(|s: &mut State| s.checked = !s.checked).build(&k))
        .child(Checkbox::new(false).label("Hidden checkbox").hide_label().build(&k))
        .child(Field::new().label("Display name").control(TextInput::new(&s.explicit)
            .on_edit(|s: &mut State| &mut s.explicit).build(&k).label("Nickname")).build(&k))
        .child(Field::new().label("Travel dates").control(row()
            .child(TextInput::new(&s.range_start).on_edit(|s: &mut State| &mut s.range_start).build(&k).label("Start date"))
            .child(TextInput::new(&s.range_end).on_edit(|s: &mut State| &mut s.range_end).build(&k).label("End date"))).build(&k));
    root(body, layers)
}

fn disputed_view(s: &State) -> El {
    let k = Kit::neutral(true);
    let control = |role: &'static str, label: &str, visible: &str, support: &str| {
        col().role(role).label(label).child(text(visible.to_owned())).child(text(support.to_owned()))
    };
    let links = rich("Read Alpha then Beta.", Vec::new()).key("proof-links").w_full().h(28.0)
        .link(Link::new(16..20, "second"))
        .link(Link::new(5..10, "first"))
        .on_link(|s: &mut State, id, _| match id {
            "first" => s.first += 1,
            "second" => s.second += 1,
            _ => {}
        });
    let scroll = col().key("touch-scroll").role("group").label("Touch scroll viewport")
        .w_full().h(140.0).scroll(ScrollSpec { stick_bottom: false })
        .on_scroll_offset(|pos| reportScroll(pos.offset[1]))
        .child(col().w_full().children((0..20).map(|i| text(format!("Scroll row {i}")).h(30.0))));
    let body = col().w_full().h_full().padding(16.0).gap(4.0)
        .child(Button::new("Before links").on_press(|s: &mut State| s.before += 1).build(&k))
        .child(links)
        .child(Button::new("After links").on_press(|s: &mut State| s.after += 1).build(&k))
        .child(col().key("launch").focusable(true).h(32.0)
            .child(text("Launch")).on_press(|s: &mut State| s.launch += 1))
        .child(col().role("listbox").child(control("option", "Option label", "Option label", "Option support")))
        .child(col().role("tablist").child(control("tab", "Tab label", "Tab label", "Tab support")))
        .child(control("checkbox", "Check label", "Check label", "Check support"))
        .child(control("radio", "Radio label", "Radio label", "Radio support"))
        .child(control("switch", "Switch label", "Switch label", "Switch support"))
        .child(col().role("listbox").child(control("option", "Distinct option name", "Visible option", "Visible support")))
        .child(col().role("tablist").child(control("tab", "Distinct tab name", "Visible tab", "Visible tab support")))
        .child(Field::new().label("Captioned editor").control(
            TextInput::new(&s.captioned).on_edit(|s: &mut State| &mut s.captioned).build(&k)
        ).build(&k))
        .child(scroll);
    root(body, Vec::new())
}

pub fn run() {
    let mut spec = AppSpec::new("Input proof", (390.0, 760.0), State::default, view);
    spec.poll = Some(|s, _, _| {
        report(&s.plain.text, &s.plain.display().0, &s.single.text, s.secret.text.chars().count(),
            s.submits, s.shortcuts, s.handle.as_ref().is_some_and(AppHandle::is_visible));
        reportActions(s.first, s.second, s.launch, s.before, s.after);
        false
    });
    spec.on_key = Some(|s, event, _| {
        if event.key == Key::Command('k') { s.shortcuts += 1; true } else { false }
    });
    let app = App::new(spec);
    let handle = app.handle();
    let state_handle = handle.clone();
    handle.update(move |s, _| s.handle = Some(state_handle));
    handle.show();
    handle.set_position((0, 0)); // Must warn, never cover the canvas with an error page.
    app.run();
}
