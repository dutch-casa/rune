//! Field (`@opencode/ui` forms/field): label (with optional info icon), prefix, control, suffix.

use rune::{El, Icon, col, icon, row, text};

use crate::Kit;

pub static INFO: Icon = Icon {
    name: "kit-field-info",
    view: 16.0,
    body: r#"<path fill-rule="evenodd" d="M13 13H3V3H13V13ZM6.46777 6.81641V7.81641H7.5791V11.3721H8.5791V6.81641H6.46777ZM7.30078 4.62891V5.62891H8.85645V4.62891H7.30078Z" fill="currentColor"/>"#,
};

pub struct Field {
    label: Option<String>,
    info: bool,
    prefix: Option<String>,
    suffix: Option<String>,
    hint: Option<String>,
    error: Option<String>,
    control: Option<El>,
}

impl Field {
    pub fn new() -> Field {
        Field { label: None, info: false, prefix: None, suffix: None, hint: None, error: None, control: None }
    }
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }
    /// Show the info icon after the label (its tooltip is a `Tooltip` the app mounts).
    pub fn info(mut self) -> Self {
        self.info = true;
        self
    }
    pub fn prefix(mut self, p: impl Into<String>) -> Self {
        self.prefix = Some(p.into());
        self
    }
    pub fn suffix(mut self, s: impl Into<String>) -> Self {
        self.suffix = Some(s.into());
        self
    }
    /// Extra description announced with the control (`aria-describedby`). An empty string clears it.
    pub fn hint(mut self, h: impl Into<String>) -> Self {
        let h = h.into();
        self.hint = (!h.is_empty()).then_some(h);
        self
    }
    /// Marks the control invalid and announces `message` with it. An empty string clears both.
    pub fn error(mut self, message: impl Into<String>) -> Self {
        let message = message.into();
        self.error = (!message.is_empty()).then_some(message);
        self
    }
    /// The field's control (a `TextInput`, `TextArea`, …). [`Self::error`] marks it invalid;
    /// [`crate::TextInput::invalid`] paints invalid without an error string.
    pub fn control(mut self, c: El) -> Self {
        self.control = Some(c);
        self
    }

    pub fn build(self, k: &Kit) -> El {
        let c = &k.c;
        let mut root = col().role("group").gap(8.0).w_full().items_start();
        if let Some(l) = &self.label {
            let mut lab = row().key("label").w_full().min_h(16.0).gap(4.0).center_y().child(
                text(l.clone()).key("text").role("label").font(rune::TextStyle { line_height: 16.0, ..k.ty.ui_medium }).color(c.text_base),
            );
            if self.info {
                lab = lab.child(icon(&INFO, 16.0).key("info").color(c.icon_muted));
            }
            root = root.child(lab);
        }
        let aside = |key: &str, s: String| {
            text(s).key(key).role("none").w_full().min_h(11.0).font(k.ty.small.lh(11.0)).color(c.text_muted)
        };
        let described: Vec<String> = [self.prefix.clone(), self.suffix.clone(), self.hint.clone(), self.error.clone()]
            .into_iter()
            .flatten()
            .filter(|s| !s.is_empty())
            .collect();
        if let Some(p) = self.prefix {
            root = root.child(aside("prefix", p));
        }
        if let Some(mut control) = self.control {
            if self.error.is_some() {
                control = control.invalid(true);
            }
            if !described.is_empty() {
                control = control.help(described.join(" "));
            }
            control = match self.label.as_deref() {
                Some(label) => control.label_if_missing(label),
                None => control,
            };
            root = root.child(control);
        }
        if let Some(s) = self.suffix {
            root = root.child(aside("suffix", s));
        }
        if let Some(h) = self.hint {
            root = root.child(aside("hint", h));
        }
        if let Some(e) = self.error {
            root = root.child(
                text(e).key("error").role("none").w_full().min_h(11.0).font(k.ty.small.lh(11.0)).color(c.danger.fg),
            );
        }
        root
    }
}

impl Default for Field {
    fn default() -> Self {
        Self::new()
    }
}
