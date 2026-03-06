use serde::{Deserialize, Serialize};

/// Token budget accounting for a response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetReport {
    /// Approximate tokens used in this response.
    pub used: u32,
    /// Effective budget for this call.
    pub limit: u32,
    /// Server-enforced maximum.
    pub hard_cap: u32,
}

/// Default token budgets per detail tier.
#[derive(Debug, Clone, Copy)]
pub struct SnapshotBudgetDefaults;

impl SnapshotBudgetDefaults {
    pub const SUMMARY: u32 = 500;
    pub const STANDARD: u32 = 1500;
    pub const FULL: u32 = 3000;
    pub const HARD_CAP: u32 = 5000;
}

/// Estimate token count from JSON byte size.
/// Rough approximation: 1 token ≈ 4 bytes of JSON.
pub fn estimate_tokens(json_bytes: usize) -> u32 {
    (json_bytes / 4) as u32
}

/// Resolve the effective budget given an optional user-requested budget,
/// a detail tier default, and the hard cap.
pub fn resolve_budget(requested: Option<u32>, tier_default: u32, hard_cap: u32) -> u32 {
    let effective = requested.unwrap_or(tier_default);
    effective.min(hard_cap)
}

/// Budget enforcer that tracks cumulative token usage and signals truncation.
pub struct BudgetEnforcer {
    limit: u32,
    hard_cap: u32,
    used_bytes: usize,
}

impl BudgetEnforcer {
    pub fn new(limit: u32, hard_cap: u32) -> Self {
        Self { limit, hard_cap, used_bytes: 0 }
    }

    /// Check if adding `bytes` would exceed the budget.
    /// Returns true if the item fits within budget.
    pub fn try_add(&mut self, bytes: usize) -> bool {
        let projected = estimate_tokens(self.used_bytes + bytes);
        if projected > self.limit {
            return false;
        }
        self.used_bytes += bytes;
        true
    }

    /// Build the budget report.
    pub fn report(&self) -> BudgetReport {
        BudgetReport {
            used: estimate_tokens(self.used_bytes),
            limit: self.limit,
            hard_cap: self.hard_cap,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_tokens_from_bytes() {
        assert_eq!(estimate_tokens(400), 100);
        assert_eq!(estimate_tokens(0), 0);
        assert_eq!(estimate_tokens(4), 1);
        assert_eq!(estimate_tokens(3), 0); // truncating division
    }

    #[test]
    fn resolve_budget_defaults() {
        assert_eq!(resolve_budget(None, 1500, 5000), 1500);
    }

    #[test]
    fn resolve_budget_explicit() {
        assert_eq!(resolve_budget(Some(3000), 1500, 5000), 3000);
    }

    #[test]
    fn resolve_budget_clamped() {
        assert_eq!(resolve_budget(Some(8000), 1500, 5000), 5000);
    }

    #[test]
    fn enforcer_tracks_budget() {
        let mut e = BudgetEnforcer::new(100, 5000);
        // 200 bytes = 50 tokens, fits in 100-token budget
        assert!(e.try_add(200));
        assert_eq!(e.report().used, 50);
    }

    #[test]
    fn enforcer_rejects_when_exceeded() {
        let mut e = BudgetEnforcer::new(10, 5000);
        // 40 bytes = 10 tokens, exactly at limit
        assert!(e.try_add(40));
        // Any more should be rejected
        assert!(!e.try_add(4));
    }
}
