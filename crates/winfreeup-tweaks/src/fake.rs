//! Hệ thống giả cho test: registry, dịch vụ, tác vụ, gói app trong bộ nhớ. Không chạm máy thật.
#![allow(dead_code)]
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use crate::model::StartType;
use crate::ops::{PackageInfo, RegData, SystemInfo, TweakOps};

pub struct FakeOps {
    pub reg: Mutex<HashMap<(String, String), RegData>>,
    pub services: Mutex<HashMap<String, StartType>>,
    pub tasks: Mutex<HashMap<String, bool>>,
    pub packages: Mutex<Vec<PackageInfo>>,
    pub sys: SystemInfo,
    /// Tên thao tác sẽ hỏng, dạng `"<hàm>:<khoá>"`, vd `"reg_write:HKLM\X|V"`, `"remove_package:A_1"`.
    pub fail: HashSet<String>,
    pub calls: Mutex<Vec<String>>,
}

impl Default for FakeOps {
    fn default() -> Self {
        FakeOps {
            reg: Mutex::default(),
            services: Mutex::default(),
            tasks: Mutex::default(),
            packages: Mutex::default(),
            sys: SystemInfo { build: 26100, edition: "Pro".into(), managed: false, other_user: false },
            fail: HashSet::new(),
            calls: Mutex::default(),
        }
    }
}

fn key(path: &str, name: &str) -> (String, String) {
    (path.to_ascii_lowercase(), name.to_ascii_lowercase())
}

impl FakeOps {
    pub fn with_reg(self, path: &str, name: &str, data: RegData) -> Self {
        self.reg.lock().unwrap().insert(key(path, name), data);
        self
    }
    pub fn with_service(self, name: &str, start: StartType) -> Self {
        self.services.lock().unwrap().insert(name.to_ascii_lowercase(), start);
        self
    }
    pub fn with_task(self, path: &str, enabled: bool) -> Self {
        self.tasks.lock().unwrap().insert(path.to_ascii_lowercase(), enabled);
        self
    }
    /// Gói bình thường của người dùng: full name = `<family>!1`.
    pub fn with_package(self, family: &str) -> Self {
        self.packages.lock().unwrap().push(PackageInfo {
            full_name: format!("{family}!1"),
            family: family.into(),
            is_framework: false,
            non_removable: false,
        });
        self
    }
    pub fn failing(mut self, what: &str) -> Self {
        self.fail.insert(what.to_string());
        self
    }
    pub fn get(&self, path: &str, name: &str) -> Option<RegData> {
        self.reg.lock().unwrap().get(&key(path, name)).cloned()
    }
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
    fn call(&self, what: String) -> Result<(), String> {
        self.calls.lock().unwrap().push(what.clone());
        if self.fail.contains(&what) {
            Err(format!("fake failure: {what}"))
        } else {
            Ok(())
        }
    }
}

impl TweakOps for FakeOps {
    fn reg_read(&self, path: &str, name: &str) -> Result<Option<RegData>, String> {
        if self.fail.contains(&format!("reg_read:{path}|{name}")) {
            return Err(format!("fake failure: reg_read:{path}|{name}"));
        }
        Ok(self.get(path, name))
    }
    fn reg_write(&self, path: &str, name: &str, data: &RegData) -> Result<(), String> {
        self.call(format!("reg_write:{path}|{name}"))?;
        self.reg.lock().unwrap().insert(key(path, name), data.clone());
        Ok(())
    }
    fn reg_delete(&self, path: &str, name: &str) -> Result<(), String> {
        self.call(format!("reg_delete:{path}|{name}"))?;
        self.reg.lock().unwrap().remove(&key(path, name));
        Ok(())
    }
    fn service_start(&self, name: &str) -> Result<Option<StartType>, String> {
        Ok(self.services.lock().unwrap().get(&name.to_ascii_lowercase()).copied())
    }
    fn set_service_start(&self, name: &str, start: StartType) -> Result<(), String> {
        self.call(format!("set_service_start:{name}:{start:?}"))?;
        self.services.lock().unwrap().insert(name.to_ascii_lowercase(), start);
        Ok(())
    }
    fn stop_service(&self, name: &str) -> Result<(), String> {
        self.call(format!("stop_service:{name}"))
    }
    fn task_enabled(&self, path: &str) -> Result<Option<bool>, String> {
        Ok(self.tasks.lock().unwrap().get(&path.to_ascii_lowercase()).copied())
    }
    fn set_task_enabled(&self, path: &str, enabled: bool) -> Result<(), String> {
        self.call(format!("set_task_enabled:{path}:{enabled}"))?;
        self.tasks.lock().unwrap().insert(path.to_ascii_lowercase(), enabled);
        Ok(())
    }
    fn packages(&self, family: &str) -> Result<Vec<PackageInfo>, String> {
        Ok(self.packages.lock().unwrap().iter().filter(|p| p.family.eq_ignore_ascii_case(family)).cloned().collect())
    }
    fn remove_package(&self, full_name: &str, all_users: bool) -> Result<(), String> {
        self.call(format!("remove_package:{full_name}:{all_users}"))?;
        self.packages.lock().unwrap().retain(|p| p.full_name != full_name);
        Ok(())
    }
    fn deprovision(&self, family: &str) -> Result<(), String> {
        self.call(format!("deprovision:{family}"))
    }
    fn open_uri(&self, uri: &str) -> Result<(), String> {
        self.call(format!("open_uri:{uri}"))
    }
    fn system_info(&self) -> Result<SystemInfo, String> {
        Ok(self.sys.clone())
    }
    fn restart_explorer(&self) -> Result<(), String> {
        self.call("restart_explorer".into())
    }
}
