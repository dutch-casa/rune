use std::borrow::Cow;
use rune::{AppSpec, Cx, El, TextEdit, col, text};
use rune_kit::{Accordion, AccordionItem, Button, Cell, Collapsible, Combobox, ComboboxOption, ComboboxState,
    Field, FileNode, FileTree, Kit, Progress, ProgressCircle, Table, TableColumn, TableState, TextInput, Toast, Tree, TreeModel, root};
use rune_kit::shell::{PaneState, Sidebar, SidebarHeader, SidebarRow, SplitView};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = "
export function accessibility_enabled() { return new URLSearchParams(location.search).has('accessibility-proof'); }
export function widget() { return new URLSearchParams(location.search).get('widget') || 'controls'; }
")]
extern "C" {
    pub fn accessibility_enabled() -> bool;
    fn widget() -> String;
}

struct State {
    widget: String,
    expanded: bool,
    toast: bool,
    edit: TextEdit,
    combo: ComboboxState,
    choice: String,
    rows: Vec<&'static str>,
    actions: u32,
    table: TableState,
    files: TreeModel<FileNode<()>>,
    sidebar: PaneState,
    inspector: PaneState,
}
impl Default for State {
    fn default() -> Self {
        let mut files = FileTree::new().compact_folders(false).build([("src/main.rs", ()), ("src/lib.rs", ()), ("README.md", ())]);
        files.expand_all();
        let widget = widget();
        let expanded = widget == "sidebar";
        Self { widget, expanded, toast: true, edit: TextEdit::new("bad"), combo: ComboboxState::default(),
            choice: "alpha".into(), table: TableState::new().multi_select(true), rows: vec!["Alpha", "Beta", "Gamma"], actions: 0, files,
            sidebar: PaneState::new(240.0), inspector: PaneState::new(200.0) }
    }
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    let k = Kit::neutral(true);
    let mut layer = Vec::new();
    let content = match s.widget.as_str() {
        "controls" => col().w_full().gap(16.0)
            .child(Accordion::new().item(AccordionItem::new("Details").expanded(s.expanded)
                .on_toggle(|s: &mut State| s.expanded = !s.expanded).child(text("Disclosure content"))).build(&k))
            .child(Collapsible::new("More", s.expanded).on_toggle(|s: &mut State| s.expanded = !s.expanded)
                .child(text("More content")).build(&k))
            .child(Progress::new(Some(0.4)).label("Upload").show_value().build(&k))
            .child(ProgressCircle::new(0.25).build(&k))
            .child(Field::new().label("Account name").suffix("Enter ready to continue")
                .control(TextInput::new(&s.edit).invalid(s.edit.text != "ready")
                    .on_edit(|s: &mut State| &mut s.edit).build(&k)).build(&k))
            .children(s.toast.then(|| Toast::new("saved", "Saved").description("Draft stored")
                .on_close(|s: &mut State| s.toast = false).build(&k))),
        "combobox" => {
            let options = [ComboboxOption::new("alpha", "Alpha"), ComboboxOption::new("beta", "Beta"), ComboboxOption::new("gamma", "Gamma")];
            Field::new().label("Model").control(Combobox::new("models", &s.combo, &options, |s: &mut State| &mut s.combo)
                .current(Some(&s.choice)).on_choose(|s, id, _| s.choice = id.into()).build(&k, &mut layer)).build(&k)
        }
        "tree" => Tree::new(|s: &State| &s.files, |s: &mut State| &mut s.files, |n: &FileNode<()>| n.name.as_str())
            .name("Files").build(s, &k).h(240.0),
        "table" => Table::new(|s: &State| &s.table, |s: &mut State| &mut s.table, |s: &State| s.rows.len(),
                |s: &State, i| Cow::Borrowed(s.rows[i]))
            .name("Records").column(TableColumn::new("name", "Name").flex(1.0).sortable())
            .cell(|s, i, _| Cell::text(s.rows[i])).on_sort(|s: &mut State, _, dir, _| s.rows.sort_by(|a, b| dir.apply(a.cmp(b))))
            .build(s, &k).h(240.0),
        "sidebar" => {
            let kit = k.clone();
            Sidebar::new().virtual_rows(if s.expanded { 2 } else { 1 }, |_: &State, i| Cow::Borrowed(if i == 0 { "head" } else { "row" }),
                move |s: &State, i| if i == 0 {
                    SidebarHeader::new("Today").collapsed(!s.expanded).on_toggle(|s: &mut State, _| s.expanded = !s.expanded).build(&kit)
                } else {
                    SidebarRow::new("Alpha").selected(true).on_select(|s: &mut State, _| s.choice = "alpha".into()).build(&kit)
                }).build(&k).h(240.0)
        }
        "split" => SplitView::new(text("Main content").color(k.c.text_base))
            .sidebar(&s.sidebar, |s: &mut State| &mut s.sidebar, text("Sidebar content").color(k.c.text_base))
            .inspector(&s.inspector, |s: &mut State| &mut s.inspector, text("Inspector content").color(k.c.text_base))
            .build(&k).grow(1.0).min_h(0.0),
        _ => text("Unknown accessibility fixture"),
    };
    root(col().w_full().h_full().padding(16.0).gap(16.0).fill(k.c.bg_base)
        .child(content).child(Button::new(format!("After controls ({})", s.actions)).on_press(|s: &mut State| s.actions += 1).build(&k)), layer)
}

pub fn run() {
    rune::run(AppSpec::new("Kit accessibility", (390.0, 760.0), State::default, view));
}
