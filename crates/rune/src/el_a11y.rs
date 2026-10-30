//! Authored accessibility state. Platform adapters read this state; they do not infer it from paint.
use crate::El;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct CheckState {
    pub checked: bool,
    pub indeterminate: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum LabelSource {
    Fallback,
    Composed,
    #[default]
    Explicit,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct A11y {
    pub level: Option<u32>,
    pub expanded: Option<bool>,
    pub secure: bool,
    pub disabled: bool,
    pub label_source: LabelSource,
    /// Absent when no checked state has been declared.
    pub checked: Option<CheckState>,
}

impl El {
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self.a11y.label_source = LabelSource::Explicit;
        self
    }
    /// Declare whether this element is checked.
    pub fn checked(mut self, checked: bool) -> Self {
        self.a11y.checked.get_or_insert_default().checked = checked;
        self
    }

    /// Declare whether this element has a mixed checked state.
    pub fn indeterminate(mut self, indeterminate: bool) -> Self {
        self.a11y.checked.get_or_insert_default().indeterminate = indeterminate;
        self
    }

    /// Disable input for this element and its descendants without hiding their semantics.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.a11y.disabled = disabled;
        self
    }

    /// Supply a default name that a composed field can replace.
    pub fn fallback_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self.a11y.label_source = LabelSource::Fallback;
        self
    }

    /// Supply a composed field's name without replacing an explicit control name.
    pub fn label_if_missing(mut self, label: impl AsRef<str>) -> Self {
        let label = label.as_ref();
        if !label.is_empty() && (self.a11y.label_source == LabelSource::Fallback || self.label.as_deref().is_none_or(str::is_empty)) {
            self.label = Some(label.to_owned());
            self.a11y.label_source = LabelSource::Composed;
        }
        self
    }
}
