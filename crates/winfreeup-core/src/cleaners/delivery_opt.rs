use crate::env::Env;
use crate::fsclean::{FileCleaner, Target};
use crate::types::RiskLevel;

pub const CACHE_REL: &str = r"ServiceProfiles\NetworkService\AppData\Local\Microsoft\Windows\DeliveryOptimization\Cache";

pub fn targets(env: &Env) -> Vec<Target> {
    vec![Target::all(env.windir.join(CACHE_REL))]
}

pub fn cleaner() -> FileCleaner {
    FileCleaner { id: "delivery_opt", risk: RiskLevel::Safe, default_selected: true, targets }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{fake_env, write_file, FakeSys};
    use crate::types::{CancelToken, Cleaner};
    use std::sync::Arc;

    #[test]
    fn metadata_matches_spec_table() {
        let c = cleaner();
        assert_eq!((c.id, c.risk, c.default_selected), ("delivery_opt", RiskLevel::Safe, true));
    }

    #[test]
    fn scans_delivery_optimization_cache() {
        let t = tempfile::tempdir().unwrap();
        let env = fake_env(t.path(), Arc::new(FakeSys::default()));
        write_file(&env.windir.join(CACHE_REL).join("ab").join("blob"), 42);
        assert_eq!(cleaner().scan(&env, &CancelToken::new()).unwrap().total_bytes, 42);
    }
}
