use crate::env::Env;
use crate::fsclean::{FileCleaner, Target};
use crate::types::RiskLevel;

pub fn targets(env: &Env) -> Vec<Target> {
    let la = &env.local_appdata;
    let pd = &env.program_data;
    let w = &env.windir;
    vec![
        Target::prefixed(la.join(r"Microsoft\Windows\Explorer"), "thumbcache_"),
        Target::all(pd.join(r"Microsoft\Windows\WER\ReportArchive")),
        Target::all(pd.join(r"Microsoft\Windows\WER\ReportQueue")),
        Target::all(la.join(r"Microsoft\Windows\WER\ReportArchive")),
        Target::all(la.join(r"Microsoft\Windows\WER\ReportQueue")),
        Target::all(la.join("CrashDumps")),
        Target::all(w.join("Minidump")),
        Target::all(w.join("MEMORY.DMP")),
    ]
}

pub fn cleaner() -> FileCleaner {
    FileCleaner { id: "win_caches", risk: RiskLevel::Safe, default_selected: true, targets }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{fake_env, write_file, FakeSys};
    use crate::types::{CancelToken, CleanOptions, Cleaner, NoProgress};
    use std::sync::Arc;

    #[test]
    fn metadata_matches_spec_table() {
        let c = cleaner();
        assert_eq!((c.id, c.risk, c.default_selected), ("win_caches", RiskLevel::Safe, true));
    }

    #[test]
    fn cleans_thumbnails_wer_and_dumps_but_not_neighbours() {
        let t = tempfile::tempdir().unwrap();
        let env = fake_env(t.path(), Arc::new(FakeSys::default()));
        let la = env.local_appdata.clone();
        let thumb = write_file(&la.join(r"Microsoft\Windows\Explorer\thumbcache_256.db"), 1);
        let icon = write_file(&la.join(r"Microsoft\Windows\Explorer\iconcache_32.db"), 1);
        let wer = write_file(&env.program_data.join(r"Microsoft\Windows\WER\ReportArchive\r1\Report.wer"), 2);
        let wer_q = write_file(&la.join(r"Microsoft\Windows\WER\ReportQueue\q1\Report.wer"), 3);
        let dump = write_file(&la.join(r"CrashDumps\app.exe.123.dmp"), 4);
        let mini = write_file(&env.windir.join(r"Minidump\092526-01.dmp"), 5);
        let memory = write_file(&env.windir.join("MEMORY.DMP"), 6);
        let other = write_file(&env.windir.join("win.ini"), 7);
        let c = cleaner();
        let scan = c.scan(&env, &CancelToken::new()).unwrap();
        assert_eq!(scan.total_bytes, 1 + 2 + 3 + 4 + 5 + 6);
        c.clean(&env, &scan, &CleanOptions::default(), &NoProgress).unwrap();
        for gone in [&thumb, &wer, &wer_q, &dump, &mini, &memory] {
            assert!(!gone.exists(), "{}", gone.display());
        }
        assert!(icon.exists() && other.exists());
        assert!(env.windir.exists());
    }

    // Review Focus 4
    #[test]
    fn machine_without_any_of_these_folders_scans_zero() {
        let t = tempfile::tempdir().unwrap();
        let env = fake_env(t.path(), Arc::new(FakeSys::default()));
        let scan = cleaner().scan(&env, &CancelToken::new()).unwrap();
        assert_eq!((scan.total_bytes, scan.file_count), (0, 0));
    }
}
