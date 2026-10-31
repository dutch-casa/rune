use crate::El;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct CheckState {
    pub checked: bool,
    pub indeterminate: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum LabelSource {
    Fallback,
    Composed,
    #[default]
    Explicit,
}

/// A numeric value and its inclusive range ([`El::value_range`]).
///
/// `El::value_range` validates finite, ordered bounds and an in-range value before
/// publishing them to the inspector and web projection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ValueRange {
    pub value: f32,
    pub min: f32,
    pub max: f32,
}

/// The orientation of a separator or range control.
///
/// Unset means the host default. A vertical splitter sets [`Orientation::Vertical`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

/// Which item is current in a set ([`El::current`]).
///
/// [`Current::True`] is the generic current item. The others name a step, a page, a place, a date
/// or a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Current {
    True,
    Page,
    Step,
    Location,
    Date,
    Time,
}

impl Current {
    pub fn as_str(self) -> &'static str {
        match self {
            Current::True => "true",
            Current::Page => "page",
            Current::Step => "step",
            Current::Location => "location",
            Current::Date => "date",
            Current::Time => "time",
        }
    }
}

/// Roles the web mirror accepts. An unknown role is omitted from the DOM node; the element stays.
/// Sorted for lookup.
#[cfg_attr(not(test), allow(dead_code))]
pub const ROLES: &[&str] = &[
    "alert", "alertdialog", "application", "article", "banner", "blockquote", "button", "caption", "cell",
    "checkbox", "code", "columnheader", "combobox", "complementary", "contentinfo", "definition", "deletion",
    "dialog", "directory", "document", "emphasis", "feed", "figure", "form", "grid", "gridcell", "group",
    "heading", "img", "insertion", "link", "list", "listbox", "listitem", "log", "main", "marquee", "math",
    "menu", "menubar", "menuitem", "menuitemcheckbox", "menuitemradio", "navigation", "none", "note", "option",
    "paragraph", "presentation", "progressbar", "radio", "radiogroup", "region", "row", "rowgroup", "rowheader",
    "scrollbar", "search", "searchbox", "separator", "slider", "spinbutton", "status", "strong", "subscript",
    "suggestion", "superscript", "switch", "tab", "table", "tablist", "tabpanel", "term", "textbox", "time",
    "timer", "toolbar", "tooltip", "tree", "treegrid", "treeitem",
];

#[cfg_attr(not(test), allow(dead_code))]
pub fn role_known(role: &str) -> bool {
    ROLES.binary_search(&role).is_ok()
}

/// Accessibility state beyond role and label.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct A11y {
    pub level: Option<u32>,
    pub expanded: Option<bool>,
    pub secure: bool,
    pub invalid: bool,
    pub label_source: LabelSource,
    pub checked: Option<CheckState>,
    pub range: Option<ValueRange>,
    /// Stable key of the controlled node. Only set by [`El::controls`].
    pub controls: Option<String>,
    pub orientation: Option<Orientation>,
    /// Toggle pressed state. `None` is not a toggle.
    pub pressed: Option<bool>,
    /// Current item in a set. `None` means not current.
    pub current: Option<Current>,
    /// The control selects more than one item (a multi-select grid, list or tree).
    pub multiselectable: bool,
}


impl El {
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self.a11y.label_source = LabelSource::Explicit;
        self
    }
    /// Declare the checked state of this element.
    pub fn checked(mut self, checked: bool) -> Self {
        self.a11y.checked.get_or_insert_default().checked = checked;
        self
    }
    /// Declare a mixed checked state.
    ///
    /// `true` marks the control mixed and creates a check state when it had none.
    /// `false` clears only the mixed flag. A control that was never checked stays
    /// not checkable.
    pub fn indeterminate(mut self, indeterminate: bool) -> Self {
        if indeterminate {
            self.a11y.checked.get_or_insert_default().indeterminate = true;
        } else if let Some(state) = self.a11y.checked.as_mut() {
            state.indeterminate = false;
        }
        self
    }
    /// Keep this element and its descendants visible and in the accessibility tree, and out of
    /// pointer and keyboard input. A click, hover, scroll or drop passes through to whatever is
    /// drawn behind, as with [`El::inert`](crate::El::inert). Tab skips the subtree, and focus
    /// already inside drops. Unlike `inert`, a disabled control stays exposed as disabled, so it
    /// can still be read. This does not change a bound editor's value. Passing `false` clears it.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    /// A placeholder a field caption may replace.
    pub fn placeholder_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self.a11y.label_source = LabelSource::Fallback;
        self
    }
    /// Supply a composed name without replacing an explicit control name.
    pub fn label_if_missing(mut self, label: impl AsRef<str>) -> Self {
        let label = label.as_ref();
        if !label.is_empty() && (self.a11y.label_source == LabelSource::Fallback || self.label.as_deref().is_none_or(str::is_empty)) {
            self.label = Some(label.to_owned());
            self.a11y.label_source = LabelSource::Composed;
        }
        self
    }
    /// An outline row's depth for accessibility, 1 at the top (ARIA `aria-level`), as a
    /// `treeitem` carries it. The element tree reports it as `level`.
    pub fn level(mut self, level: u32) -> Self {
        self.a11y.level = Some(level);
        self
    }
    /// Whether a disclosure (a tree row with children, a collapsible header) is open, for
    /// accessibility (ARIA `aria-expanded`). Leave it unset on an element that can't open.
    /// The element tree reports it as `expanded`.
    pub fn expanded(mut self, open: bool) -> Self {
        self.a11y.expanded = Some(open);
        self
    }
    /// Store a numeric value and its inclusive range.
    ///
    /// `value`, the range start and the range end must be finite, the start must not exceed
    /// the end, and `value` must lie in that range. A violated contract panics. This does not
    /// clamp. Omit the call when the control has no value.
    #[track_caller]
    pub fn value_range(mut self, value: f32, range: std::ops::RangeInclusive<f32>) -> Self {
        let (min, max) = (*range.start(), *range.end());
        assert!(
            value.is_finite() && min.is_finite() && max.is_finite() && min <= max && (min..=max).contains(&value),
            "value_range requires a finite value inside a finite ordered inclusive range and does not clamp; got {value} in {min}..={max}",
        );
        self.a11y.range = Some(ValueRange { value, min, max });
        self
    }
    /// Mark this element invalid, or clear that mark.
    ///
    /// Pass the bool the control already holds. This does not add an error string.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.a11y.invalid = invalid;
        self
    }
    /// The stable key of the node this element controls.
    ///
    /// Resolved to one current node. A missing or ambiguous key is not a relationship.
    /// An empty key clears it. Only a real controller should call this.
    pub fn controls(mut self, key: impl Into<String>) -> Self {
        let key = key.into();
        self.a11y.controls = (!key.is_empty()).then_some(key);
        self
    }
    /// The orientation of this separator or range control.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.a11y.orientation = Some(orientation);
        self
    }
    /// A toggle button's pressed state. Unset means this is not a toggle.
    pub fn pressed(mut self, pressed: bool) -> Self {
        self.a11y.pressed = Some(pressed);
        self
    }
    /// Mark this item current in a set: a step, a page, a place, a date, a time, or just current.
    pub fn current(mut self, current: Current) -> Self {
        self.a11y.current = Some(current);
        self
    }
    /// This grid, list or tree selects more than one item.
    pub fn multiselectable(mut self, on: bool) -> Self {
        self.a11y.multiselectable = on;
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::el;

    #[test]
    fn roles_stay_sorted_and_match_the_web_mirror() {
        assert!(super::ROLES.windows(2).all(|pair| pair[0] < pair[1]));
        let src = include_str!("../platform/web/bridge.js");
        let marker = "const roles = new Set(('";
        let start = src.find(marker).expect("bridge lists roles");
        let rest = &src[start + marker.len()..];
        let end = rest.find("').split").expect("role list closes");
        let js: std::collections::BTreeSet<&str> = rest[..end].split_whitespace().collect();
        let rust: std::collections::BTreeSet<&str> = super::ROLES.iter().copied().collect();
        assert_eq!(js, rust);
        assert!(super::role_known("button") && super::role_known("treeitem"));
        assert!(!super::role_known("nope") && !super::role_known("Button"));
    }

    #[test]
    fn value_range_keeps_an_inclusive_finite_range_and_rejects_the_rest() {
        let stored = el().value_range(0.0, 0.0..=100.0).value_range(100.0, 0.0..=100.0);
        assert_eq!(stored.a11y.range, Some(super::ValueRange { value: 100.0, min: 0.0, max: 100.0 }));
        for (value, min, max) in [(f32::NAN, 0.0, 1.0), (0.0, f32::INFINITY, 1.0), (0.5, 1.0, 0.0), (2.0, 0.0, 1.0)] {
            let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = el().value_range(value, min..=max);
            }));
            assert!(failed.is_err(), "{value} in {min}..={max} must be rejected");
        }
    }

    #[test]
    fn indeterminate_false_does_not_make_a_control_checkable() {
        assert!(el().indeterminate(false).a11y.checked.is_none());
        let mixed = el().checked(true).indeterminate(true).indeterminate(false);
        assert_eq!(mixed.a11y.checked, Some(CheckState { checked: true, indeterminate: false }));
        let off = el().checked(false).indeterminate(false);
        assert_eq!(off.a11y.checked, Some(CheckState { checked: false, indeterminate: false }));
        assert!(el().indeterminate(true).a11y.checked.is_some_and(|s| s.indeterminate));
    }
}
