//! Shared table-column policy, independent of GTK and source row projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    total: usize,
    limit: usize,
    selected: Vec<usize>,
}
impl Selection {
    pub fn new(total: usize, mut selected: Vec<usize>, limit: usize) -> Self {
        selected.retain(|index| *index < total);
        selected.sort_unstable();
        selected.dedup();
        let limit = limit.max(1).min(total);
        selected.truncate(limit);
        if selected.is_empty() && total > 0 {
            selected.push(0);
        }
        Self {
            total,
            limit,
            selected,
        }
    }
    pub fn selected(&self) -> &[usize] {
        &self.selected
    }
    pub fn propose(&self, index: usize, active: bool) -> Option<Self> {
        if index >= self.total {
            return None;
        }
        let position = self.selected.binary_search(&index);
        let mut next = self.clone();
        match (position, active) {
            (Ok(_), true) | (Err(_), false) => return None,
            (Ok(_), false) if self.selected.len() == 1 => return None,
            (Err(_), true) if self.selected.len() >= self.limit => return None,
            (Ok(position), false) => {
                next.selected.remove(position);
            }
            (Err(position), true) => next.selected.insert(position, index),
        }
        Some(next)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_saved_indices_are_normalized_without_reordering_columns() {
        assert_eq!(
            Selection::new(6, vec![5, 2, 2, 99, 0], 3).selected(),
            [0, 2, 5]
        );
        assert_eq!(Selection::new(6, vec![], 3).selected(), [0]);
        assert!(Selection::new(0, vec![0], 0).selected().is_empty());
    }
    #[test]
    fn changes_keep_one_column_and_respect_the_cell_budget() {
        let selection = Selection::new(600, (0..512).collect(), 512);
        assert!(selection.propose(599, true).is_none());
        let next = selection
            .propose(22, false)
            .unwrap()
            .propose(599, true)
            .unwrap();
        assert_eq!(next.selected().len(), 512);
        assert!(!next.selected().contains(&22));
        assert_eq!(next.selected().last(), Some(&599));
        let one = Selection::new(600, vec![599], 512);
        assert!(one.propose(599, false).is_none());
        assert!(one.propose(600, true).is_none());
        assert!(one.propose(599, true).is_none());
        assert!(one.propose(0, false).is_none());
    }
}
