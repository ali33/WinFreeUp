use crate::env::Env;
use crate::fsclean::{FileCleaner, Target, DAY};
use crate::types::RiskLevel;

pub fn targets(env: &Env) -> Vec<Target> {
    vec![Target::older_than(env.windir.join("Temp"), DAY)]
}

pub fn cleaner() -> FileCleaner {
    FileCleaner { id: "system_temp", risk: RiskLevel::Safe, default_selected: true, targets }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{age, fake_env, write_file, FakeSys};
    use crate::types::{CancelToken, Cleaner};
    use std::sync::Arc;

    #[test]
    fn metadata_matches_spec_table() {
        let c = cleaner();
        assert_eq!((c.id, c.risk, c.default_selected), ("system_temp", RiskLevel::Safe, true));
    }

    #[test]
    fn scans_windows_temp_older_than_24h_only() {
        let t = tempfile::tempdir().unwrap();
        let env = fake_env(t.path(), Arc::new(FakeSys::default()));
        let old = write_file(&env.windir.join("Temp").join("a.tmp"), 10);
        write_file(&env.windir.join("Temp").join("b.tmp"), 20);
        write_file(&env.temp.join("user.tmp"), 40);
        age(&old, 25);
        assert_eq!(cleaner().scan(&env, &CancelToken::new()).unwrap().total_bytes, 10);
    }
}
