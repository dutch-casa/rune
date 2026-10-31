//! What `El` holds for focus and keys: `FocusSpec`, kept behind `Cold` because most elements set none of it.
use crate::el::{InputKeyFn, StepFn, StepLimitFn};
use crate::color::Color;
use crate::el::Outline;

#[derive(Clone)]
pub(crate) struct FocusSpec {
    pub physical: Option<crate::input::physical_input::Handler>,
    pub enabled: bool,
    pub order: i32,
    pub group: bool,
    pub selected: bool,
    /// [`El::focus_selected`] was called, so this item is in a selection even when not selected.
    pub in_selection: bool,
    /// Keyboard cursor, distinct from [`El::focus_selected`].
    pub cursor: bool,
    pub reveal_selected: bool,
    pub preserve_on_press: crate::runtime::selectable::OnPress,
    pub ring: Option<Outline>,
    /// `no_focus_ring`: no default ring on this control, and none from this element.
    pub ring_off: bool,
    /// `default_button`: Return presses it.
    pub default: bool,
    pub highlight: Option<Color>,
    pub follows: bool,
    pub return_to: Option<String>,
    /// `on_step`: a slider's arrow and page keys.
    pub step: Option<StepFn>,
    /// `on_step_limit`: a slider's Home and End.
    pub step_limit: Option<StepLimitFn>,
    /// `on_key_up`.
    pub key_up: Option<InputKeyFn>,
    /// `El::on_key`: keys while this element or one inside it has focus.
    pub key: Option<InputKeyFn>,
}

impl crate::cold::Empty for FocusSpec {
    const EMPTY: &'static Self = &FocusSpec {
        physical: None,
        enabled: false,
        order: 0,
        group: false,
        selected: false,
        in_selection: false,
        reveal_selected: false,
        cursor: false,
        preserve_on_press: crate::runtime::selectable::OnPress { focus: false, selection: false },
        ring: None,
        ring_off: false,
        default: false,
        highlight: None,
        follows: false,
        return_to: None,
        step: None,
        step_limit: None,
        key_up: None,
        key: None,
    };
}

impl crate::El {
    /// Mark the keyboard cursor. This is not selection ([`El::focus_selected`]).
    ///
    /// A list, tree, grid, or popup sets it on the row the arrow keys would move from.
    /// The inspector reports it as [`crate::ElementInfo::item_cursor`].
    pub fn focus_cursor(mut self, cursor: bool) -> Self {
        self.focus.cursor = cursor;
        self
    }
}
