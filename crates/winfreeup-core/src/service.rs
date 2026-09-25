//! Dịch vụ đã dừng thì LUÔN được bật lại (spec mục 6.5), kể cả khi lỗi hoặc panic giữa chừng.
use crate::env::SystemOps;
use crate::error::Result;

pub struct ServiceGuard<'a> {
    sys: &'a dyn SystemOps,
    name: &'static str,
    restart: bool,
}

impl<'a> ServiceGuard<'a> {
    pub fn stop(sys: &'a dyn SystemOps, name: &'static str) -> Result<ServiceGuard<'a>> {
        let was_running = sys.stop_service(name)?;
        Ok(ServiceGuard { sys, name, restart: was_running })
    }

    /// Bật lại ngay và trả lỗi bật lại (nếu có) cho người gọi ghi vào báo cáo.
    pub fn finish(mut self) -> Result<()> {
        if self.restart {
            self.restart = false;
            self.sys.start_service(self.name)
        } else {
            Ok(())
        }
    }
}

impl Drop for ServiceGuard<'_> {
    fn drop(&mut self) {
        if self.restart {
            self.restart = false;
            let _ = self.sys.start_service(self.name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::FakeSys;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    #[test]
    fn finish_restarts_a_service_that_was_running() {
        let sys = FakeSys { service_was_running: true, ..Default::default() };
        ServiceGuard::stop(&sys, "wuauserv").unwrap().finish().unwrap();
        assert_eq!(sys.calls(), vec!["stop:wuauserv", "start:wuauserv"]);
    }

    #[test]
    fn service_that_was_already_stopped_is_left_stopped() {
        let sys = FakeSys { service_was_running: false, ..Default::default() };
        ServiceGuard::stop(&sys, "wuauserv").unwrap().finish().unwrap();
        assert_eq!(sys.calls(), vec!["stop:wuauserv"]);
    }

    #[test]
    fn drop_restarts_even_when_the_work_panics() {
        let sys = FakeSys { service_was_running: true, ..Default::default() };
        let r = catch_unwind(AssertUnwindSafe(|| {
            let _g = ServiceGuard::stop(&sys, "wuauserv").unwrap();
            panic!("lỗi giữa chừng");
        }));
        assert!(r.is_err());
        assert_eq!(sys.calls(), vec!["stop:wuauserv", "start:wuauserv"]);
    }

    #[test]
    fn drop_restarts_on_early_return_with_error() {
        let sys = FakeSys { service_was_running: true, ..Default::default() };
        fn work(sys: &FakeSys) -> Result<()> {
            let _g = ServiceGuard::stop(sys, "wuauserv")?;
            Err(crate::error::CoreError::System("xóa hỏng".into()))
        }
        assert!(work(&sys).is_err());
        assert_eq!(sys.calls(), vec!["stop:wuauserv", "start:wuauserv"]);
    }
}
