use libc::{CLOCK_MONOTONIC, itimerspec, timerfd_create, timerfd_settime};
use std::{
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    time::Duration,
};
use tokio::io::unix::AsyncFd;

pub struct HighResultionTimer {
    timer_fd: tokio::io::unix::AsyncFd<std::os::fd::OwnedFd>,
}
impl HighResultionTimer {
    pub fn interval(dur: Duration) -> Result<Self, std::io::Error> {
        let timer_fd = unsafe { timerfd_create(CLOCK_MONOTONIC, libc::TFD_CLOEXEC) };
        if timer_fd == -1 {
            return Err(std::io::Error::last_os_error());
        }

        let timer_fd = unsafe { OwnedFd::from_raw_fd(timer_fd) };
        let mut timer = Self {
            timer_fd: AsyncFd::new(timer_fd)?,
        };

        timer.set_polling_rate(dur.as_micros() as u64)?;
        Ok(timer)
    }

    /// Sets the interval, in microseconds, at which the [AsyncReceiver] polls the queue for new elements.
    /// # Errors
    /// Returns an [std::io::Error] if the underlying timer could not be reconfigured.
    fn set_polling_rate(&mut self, rate_us: u64) -> Result<(), std::io::Error> {
        let mut ts: itimerspec = unsafe { core::mem::zeroed() };

        let cycletime_ns = rate_us as i64 * 1000;

        let tv_sec = cycletime_ns / 1_000_000_000;
        let tv_nsec = cycletime_ns % 1_000_000_000;

        // First expiration after 1 second
        ts.it_value.tv_sec = tv_sec;
        ts.it_value.tv_nsec = tv_nsec;

        // Then every 500 ms
        ts.it_interval.tv_sec = tv_sec;
        ts.it_interval.tv_nsec = tv_nsec;
        let ret =
            unsafe { timerfd_settime(self.timer_fd.as_raw_fd(), 0, &ts, core::ptr::null_mut()) };

        if ret == -1 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
    pub async fn tick(&self) {
        if let Ok(guard) = self.timer_fd.readable().await {
            let mut buf = [0u8; 8];
            let _ = unsafe {
                use std::ffi::c_void;
                libc::read(
                    guard.get_inner().as_raw_fd(),
                    &raw mut buf as *mut c_void,
                    buf.len(),
                )
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::timer::HighResultionTimer;

    #[tokio::test]
    async fn smoke() {
        let timer = HighResultionTimer::interval(Duration::from_secs(1)).unwrap();
        for _ in 0..3 {
            let start = std::time::Instant::now();
            timer.tick().await;
            let end = start.elapsed();
            assert!(end.as_millis() >= 990, "{}", end.as_millis());
        }
    }
}
