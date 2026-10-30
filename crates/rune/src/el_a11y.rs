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
/// Unset means the host default. A vertical splitter sets [`Axis::Vertical`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

/// Accessibility state beyond role and label.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct A11y {
    pub level: Option<u32>,
    pub expanded: Option<bool>,
    pub secure: bool,
    pub disabled: bool,
    pub invalid: bool,
    pub label_source: LabelSource,
    pub checked: Option<CheckState>,
    pub range: Option<ValueRange>,
    /// Stable key of the controlled node. Only set by [`El::controls`].
    pub controls: Option<String>,
    pub orientation: Option<Axis>,
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
    pub fn indeterminate(mut self, indeterminate: bool) -> Self {
        self.a11y.checked.get_or_insert_default().indeterminate = indeterminate;
        self
    }
    /// Exclude this element and its descendants from hit testing and keyboard focus.
    /// Content stays visible. This does not change the bound editor's value.
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
    pub fn orientation(mut self, axis: Axis) -> Self {
        self.a11y.orientation = Some(axis);
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::el;

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
}
