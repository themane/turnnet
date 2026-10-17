//! Monotonic session counters.

macro_rules! counter {
    ($(#[$doc:meta])* $name:ident($t:ty) $(, $cname:ident = $cval:expr)*) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
        pub struct $name($t);

        impl $name {
            $(pub const $cname: Self = Self($cval);)*

            pub const fn new(value: $t) -> Self {
                Self(value)
            }

            pub const fn get(self) -> $t {
                self.0
            }

            /// The following value, or `None` on overflow.
            pub const fn next(self) -> Option<Self> {
                match self.0.checked_add(1) {
                    Some(v) => Some(Self(v)),
                    None => None,
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Display::fmt(&self.0, f)
            }
        }
    };
}

counter!(
    /// Host generation. The first host uses term 1; each migration/transfer advances it.
    Term(u64), INITIAL = 1
);
counter!(
    /// Game-state version. The initial state is version 0; each accepted action adds one.
    StateVersion(u64), INITIAL = 0
);
counter!(
    /// Committed roster generation. Starts at 1; advances only on committed roster changes.
    MembershipEpoch(u64), INITIAL = 1
);
counter!(
    /// Total-order authoritative commit sequence. The initial commit is index 1.
    CommitIndex(u64), FIRST = 1
);
counter!(
    /// Session-monotonic admission order. The creating host is 0; values are never reused.
    JoinOrder(u32), FIRST_HOST = 0
);
counter!(
    /// Per-peer route binding generation; increases on every authenticated route change.
    RouteGeneration(u64)
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_values_match_spec() {
        assert_eq!(Term::INITIAL.get(), 1);
        assert_eq!(StateVersion::INITIAL.get(), 0);
        assert_eq!(MembershipEpoch::INITIAL.get(), 1);
        assert_eq!(CommitIndex::FIRST.get(), 1);
        assert_eq!(JoinOrder::FIRST_HOST.get(), 0);
    }

    #[test]
    fn next_is_checked() {
        assert_eq!(Term::INITIAL.next(), Some(Term::new(2)));
        assert_eq!(JoinOrder::new(u32::MAX).next(), None);
        assert_eq!(CommitIndex::new(u64::MAX).next(), None);
    }
}
