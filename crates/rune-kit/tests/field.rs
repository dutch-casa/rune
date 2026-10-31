//! A field's hint, suffix and error describe the control.

use rune::{Cx, El, Env, Runtime, TextEdit};
use rune_kit::{Field, Kit, TextInput};

#[test]
fn a_fields_hint_and_error_describe_the_control() {
    struct S { edit: TextEdit }
    fn view(s: &S, _: &mut Cx<S>) -> El {
        let k = Kit::neutral(true);
        Field::new()
            .label("Weight")
            .prefix("kg")
            .suffix("net")
            .hint("Include packaging")
            .error("Required")
            .control(TextInput::new(&s.edit).on_edit(|s: &mut S| &mut s.edit).build(&k))
            .build(&k)
    }
    let mut rt = Runtime::new(S { edit: TextEdit::new("") }, view, (320.0, 160.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let shell = rt.elements().into_iter().find(|e| e.role == "textbox" && e.help.is_some()).unwrap();
    assert_eq!(shell.help.as_deref(), Some("kg net Include packaging Required"));
    assert!(shell.invalid);
    assert_eq!(shell.label.as_deref(), Some("Weight"));
    let json = rt.a11y_json();
    assert!(json.contains("kg net Include packaging Required"));
    assert!(json.contains("\"invalid\":true"));
    assert!(rt.elements().iter().filter(|e| e.text.as_deref() == Some("kg")).all(|e| e.role == "none"));
}

#[test]
fn an_empty_field_error_clears_the_invalid_mark() {
    struct S { edit: TextEdit }
    fn view(s: &S, _: &mut Cx<S>) -> El {
        let k = Kit::neutral(true);
        Field::new().error("").control(TextInput::new(&s.edit).build(&k)).build(&k)
    }
    let mut rt = Runtime::new(S { edit: TextEdit::default() }, view, (200.0, 80.0), Env::new(true, 1.0));
    rt.frame(0.0);
    assert!(rt.elements().iter().all(|e| !e.invalid));
}
