//! Danh sách đầy đủ 9 nhóm, đúng thứ tự bảng spec mục 3 và `src/catalog.ts` của giao diện.
use super::{
    browser_cache, component_store, delivery_opt, recycle_bin, system_temp, user_temp, win_caches, windows_old,
    wu_download,
};
use crate::types::Cleaner;

pub const ALL_IDS: [&str; 9] = [
    "user_temp",
    "system_temp",
    "browser_cache",
    "win_caches",
    "delivery_opt",
    "wu_download",
    "component_store",
    "recycle_bin",
    "windows_old",
];

pub fn all_cleaners() -> Vec<Box<dyn Cleaner>> {
    vec![
        Box::new(user_temp::cleaner()),
        Box::new(system_temp::cleaner()),
        Box::new(browser_cache::BrowserCache),
        Box::new(win_caches::cleaner()),
        Box::new(delivery_opt::cleaner()),
        Box::new(wu_download::WuDownload),
        Box::new(component_store::ComponentStore),
        Box::new(recycle_bin::RecycleBin),
        Box::new(windows_old::WindowsOld),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RiskLevel::{self, *};

    #[test]
    fn registry_matches_spec_table_in_order() {
        let expected: [(&str, RiskLevel, bool); 9] = [
            ("user_temp", Safe, true),
            ("system_temp", Safe, true),
            ("browser_cache", Safe, true),
            ("win_caches", Safe, true),
            ("delivery_opt", Safe, true),
            ("wu_download", Caution, false),
            ("component_store", Caution, false),
            ("recycle_bin", Caution, false),
            ("windows_old", Risky, false),
        ];
        let all = all_cleaners();
        let got: Vec<_> = all.iter().map(|c| (c.id(), c.risk(), c.default_selected())).collect();
        assert_eq!(got, expected.to_vec());
        assert_eq!(all.iter().map(|c| c.id()).collect::<Vec<_>>(), ALL_IDS.to_vec());
    }
}
