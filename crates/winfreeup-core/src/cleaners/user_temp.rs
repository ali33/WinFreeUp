use crate::env::Env;
use crate::fsclean::{FileCleaner, Target, DAY};
use crate::types::RiskLevel;

pub fn targets(env: &Env) -> Vec<Target> {
    vec![Target::older_than(env.temp.clone(), DAY)]
}

pub fn cleaner() -> FileCleaner {
    FileCleaner { id: "user_temp", risk: RiskLevel::Safe, default_selected: true, targets }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{age, fake_env, write_file, FakeSys};
    use crate::types::{CancelToken, CleanOptions, Cleaner, NoProgress};
    use std::sync::Arc;

    #[test]
    fn metadata_matches_spec_table() {
        let c = cleaner();
        assert_eq!((c.id, c.risk, c.default_selected), ("user_temp", RiskLevel::Safe, true));
    }

    #[test]
    fn cleans_only_user_temp_files_older_than_24h() {
        let t = tempfile::tempdir().unwrap();
        let env = fake_env(t.path(), Arc::new(FakeSys::default()));
        let old = write_file(&env.temp.join("setup.log"), 100);
        let new = write_file(&env.temp.join("dang-dung.tmp"), 50);
        age(&old, 30);
        let c = cleaner();
        let scan = c.scan(&env, &CancelToken::new()).unwrap();
        assert_eq!(scan.total_bytes, 100);
        c.clean(&env, &scan, &CleanOptions::default(), &NoProgress).unwrap();
        assert!(!old.exists() && new.exists());
    }
}
