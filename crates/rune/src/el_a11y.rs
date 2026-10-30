use crate::{El, el::LabelSource};

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
    pub fn indeterminate(mut self, indeterminate: bool) -> Self {
        self.a11y.checked.get_or_insert_default().indeterminate = indeterminate;
        self
    }
    /// Disable input without hiding this element or its descendants.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.a11y.disabled = disabled;
        self
    }
    /// Replace any existing name with a fallback that a composed field can replace.
    pub fn fallback_label(mut self, label: impl Into<String>) -> Self {
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
}
