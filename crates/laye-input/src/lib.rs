use std::collections::HashSet;

#[derive(Default, Debug, Clone)]
pub struct InputClaims(HashSet<&'static str>);

impl InputClaims {
    pub fn claim(&mut self, who: &'static str) {
        self.0.insert(who);
    }

    pub fn release(&mut self, who: &'static str) {
        self.0.remove(who);
    }

    pub fn release_all(&mut self) {
        self.0.clear();
    }

    pub fn is_captured(&self) -> bool {
        !self.0.is_empty()
    }

    pub fn claimants(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.0.iter().copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Intent {
    ChatFocus,
    DrawerToggle,
    Screenshot,
    InventoryToggle,
    ReleaseAll,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_not_captured() {
        let c = InputClaims::default();
        assert!(!c.is_captured());
        assert_eq!(c.claimants().count(), 0);
    }

    #[test]
    fn claim_marks_captured() {
        let mut c = InputClaims::default();
        c.claim("chat");
        assert!(c.is_captured());
        assert!(c.claimants().any(|k| k == "chat"));
    }

    #[test]
    fn release_removes_claimant() {
        let mut c = InputClaims::default();
        c.claim("chat");
        c.release("chat");
        assert!(!c.is_captured());
    }

    #[test]
    fn release_unknown_is_noop() {
        let mut c = InputClaims::default();
        c.release("nobody");
        assert!(!c.is_captured());
    }

    #[test]
    fn claim_is_idempotent() {
        let mut c = InputClaims::default();
        c.claim("chat");
        c.claim("chat");
        c.release("chat");
        assert!(!c.is_captured());
    }

    #[test]
    fn multiple_claimants_compose() {
        let mut c = InputClaims::default();
        c.claim("chat");
        c.claim("wardrobe");
        c.release("chat");
        assert!(c.is_captured());
        c.release("wardrobe");
        assert!(!c.is_captured());
    }

    #[test]
    fn release_all_clears_every_claim() {
        let mut c = InputClaims::default();
        c.claim("chat");
        c.claim("wardrobe");
        c.claim("obelisk-plaque");
        c.release_all();
        assert!(!c.is_captured());
        assert_eq!(c.claimants().count(), 0);
    }

    #[test]
    fn intent_is_copy_and_hashable() {
        let a = Intent::ChatFocus;
        let b = a;
        assert_eq!(a, b);
        let mut set = HashSet::new();
        set.insert(Intent::Screenshot);
        set.insert(Intent::Screenshot);
        assert_eq!(set.len(), 1);
    }
}
