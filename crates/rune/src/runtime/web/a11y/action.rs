//! Actions from the semantic DOM, checked against the current tree.
impl<S: 'static> crate::Runtime<S> {
    /// DOM presence does not authorize an action on a changed scene.
    pub(crate) fn web_a11y_action(&mut self, event: &serde_json::Value) -> bool {
        if event["kind"] == "a11y_blur" {
            if self.set_focus(None, true) { self.dirty = true; }
            return true;
        }
        let Some(path) = event["path"].as_str() else { return false };
        let Some(i) = self.live(path) else { return false };
        if !self.allowed(i) || self.subtree_exiting(i) || self.subtree_unmounted(i)
            || std::iter::successors(Some(i), |&j| self.nodes[j].parent).any(|j| self.nodes[j].vis.opacity <= 0.01)
        { return false; }
        let rect = super::rect4(self.drawn_box(i));
        if rect[2] <= 0.0 || rect[3] <= 0.0 { return false; }
        match event["kind"].as_str() {
            Some("a11y_focus") => {
                if !event["link"].is_null() {
                    let Some(k) = event["link"].as_u64().and_then(|n| usize::try_from(n).ok()) else { return false };
                    if !self.link_eligible(i, k) { return false; }
                    return self.focus_link(i, k);
                }
                if !self.can_focus(i) { return false; }
                if self.set_focus(Some(path.to_owned()), true) { self.dirty = true; }
                self.pending.scroll_to.push(super::ScrollReveal::into_view(None, path.to_owned()));
                true
            }
            Some("a11y_press") if self.press_capable(i) => {
                self.queue.push(super::Action::Press(path.to_owned(), crate::PointerButton::Primary));
                true
            }
            Some("a11y_link") => {
                let Some(k) = event["link"].as_u64().and_then(|n| usize::try_from(n).ok()) else { return false };
                if !self.link_eligible(i, k) || self.nodes[i].text.as_ref().is_none_or(|t| t.on_link.is_none()) { return false; }
                self.queue.push(super::Action::Link(path.to_owned(), k));
                true
            }
            _ => false,
        }
    }
}
