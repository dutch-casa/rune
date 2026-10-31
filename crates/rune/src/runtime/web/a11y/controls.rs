//! Which node a control names.
impl<S: 'static> crate::Runtime<S> {
    /// One prepass from key and path to node. Resolving each controller then walks only that key's chain.
    pub(in crate::runtime) fn control_facts(&self) -> (Vec<Option<String>>, Vec<bool>) {
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
}
