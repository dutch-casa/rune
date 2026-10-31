//! Which node a control names, and which cursor that owner points at.
use super::A11yItem;

impl<S: 'static> crate::Runtime<S> {
    /// One prepass from key and path to node. Resolving each controller then walks only that key's chain.
    pub(super) fn control_facts(&self) -> (Vec<Option<String>>, Vec<bool>) {
        let n = self.nodes.len();
        let mut path = vec![None; n];
        let mut target = vec![false; n];
        if !self.nodes.iter().any(|node| node.a11y.controls.is_some()) { return (path, target); }
        let mut first = std::collections::HashMap::<&str, u32>::new();
        let mut next = vec![u32::MAX; n];
        let mut by_path = std::collections::HashMap::<&str, u32>::with_capacity(n);
        for (i, node) in self.nodes.iter().enumerate().rev() {
            by_path.insert(node.path.as_str(), i as u32);
            if let Some(key) = node.key.as_deref() {
                next[i] = first.insert(key, i as u32).unwrap_or(u32::MAX);
            }
        }
        let live: Vec<bool> = (0..n).map(|i| self.control_current(i)).collect();
        for i in 0..n {
            let Some(selector) = self.nodes[i].a11y.controls.as_deref() else { continue; };
            let mut found = None;
            let mut consider = |j: usize| -> bool {
                if !live[j] { return true; }
                match found {
                    None => { found = Some(j); true }
                    Some(prev) if prev == j => true,
                    Some(_) => false,
                }
            };
            let mut ambiguous = false;
            if let Some(&j) = by_path.get(selector) {
                ambiguous = !consider(j as usize);
            }
            if !ambiguous {
                let tails = std::iter::once(selector).chain(selector.match_indices('/').map(|(at, _)| &selector[at + 1..]));
                'tails: for tail in tails {
                    let Some(&start) = first.get(tail) else { continue; };
                    let mut j = start;
                    while j != u32::MAX {
                        let candidate = &self.nodes[j as usize];
                        let matches = crate::flight::selects(selector, &candidate.path, candidate.key.as_deref());
                        if matches && !consider(j as usize) {
                            ambiguous = true;
                            break 'tails;
                        }
                        j = next[j as usize];
                    }
                }
            }
            if ambiguous { continue; }
            if let Some(j) = found {
                path[i] = Some(self.nodes[j].path.clone());
                target[j] = true;
            }
        }
        (path, target)
    }

    pub(in super::super) fn control_paths(&self) -> Vec<Option<String>> {
        self.control_facts().0
    }

    fn control_current(&self, i: usize) -> bool {
        self.in_scope(i) && !self.subtree_exiting(i) && !self.subtree_unmounted(i)
            && std::iter::successors(Some(i), |&j| self.nodes[j].parent).all(|j| self.nodes[j].vis.opacity > 0.01)
    }

    /// Point a keyboard owner at its cursor-marked mounted descendant.
    ///
    /// Selection is not the cursor. A nearer focusable grid or tree keeps its own row.
    /// An editor points at a cursor only when that cursor is owned by the element `controls` names.
    /// Two cursors under one owner, or a stale control, leave the relationship unset.
    pub(super) fn assign_active_descendants(&self, items: &mut [A11yItem], origins: &[usize]) {
        debug_assert_eq!(items.len(), origins.len());
        let n = self.nodes.len();
        let mut keyboard_owner = vec![None; n];
        for i in 0..n {
            let parent = self.nodes[i].parent;
            let role = self.nodes[i].role;
            keyboard_owner[i] = if matches!(role, "tree" | "treegrid" | "grid") && self.can_focus(i) {
                Some(i)
            } else {
                parent.and_then(|p| keyboard_owner[p])
            };
        }
        #[derive(Clone, Copy)]
        enum Slot { Empty, One(usize), Many }
        let mut slot = vec![Slot::Empty; n];
        for (k, &origin) in origins.iter().enumerate() {
            if !self.nodes[origin].focus.cursor { continue; }
            let Some(owner) = keyboard_owner[origin].filter(|&owner| owner != origin) else { continue; };
            slot[owner] = match slot[owner] {
                Slot::Empty => Slot::One(k),
                Slot::One(_) | Slot::Many => Slot::Many,
            };
        }
        for (k, &origin) in origins.iter().enumerate() {
            if let Slot::One(desc) = slot[origin] {
                items[k].active_descendant = Some(items[desc].path.clone());
            }
        }
        let Some(focus) = self.focused_index().filter(|&i| self.nodes[i].input.is_some()) else { return; };
        let Some(editor) = origins.iter().position(|&origin| origin == focus) else { return; };
        let Some(controlled) = items[editor].controls.as_deref() else { return; };
        let by_path: std::collections::HashMap<&str, usize> = items.iter().enumerate().map(|(k, it)| (it.path.as_str(), k)).collect();
        if !by_path.contains_key(controlled) { return; }
        let mut cursor = None;
        for (k, &origin) in origins.iter().enumerate() {
            if !self.nodes[origin].focus.cursor || items[k].path == controlled { continue; }
            let mut parent = items[k].parent.as_deref();
            let mut owned = false;
            while let Some(path) = parent {
                if path == controlled { owned = true; break; }
                parent = by_path.get(path).and_then(|&j| items[j].parent.as_deref());
            }
            if !owned { continue; }
            cursor = match cursor {
                None => Some(k),
                Some(_) => return,
            };
        }
        if let Some(k) = cursor {
            items[editor].active_descendant = Some(items[k].path.clone());
        }
    }
}
