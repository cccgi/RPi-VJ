//! Hardware and simulated transport layer for Stream Deck XL.
//!
//! Separates input read channel and output write channel so stalled writes
//! never block input report reading.

use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

use crate::{device::STREAM_DECK_XL_KEY_COUNT, StreamDeckError};

/// Abstract HID transport interface with thread-safe decoupled read and write.
pub trait Transport: Send + Sync {
    /// Read next input report into `buf` with timeout.
    fn read(&self, buf: &mut [u8], timeout: Duration) -> Result<usize, StreamDeckError>;

    /// Write an output report (e.g. 1024-byte JPEG packet) with timeout.
    fn write(&self, buf: &[u8], timeout: Duration) -> Result<usize, StreamDeckError>;

    /// Send a feature report (e.g. brightness, reset) with timeout.
    fn send_feature(&self, buf: &[u8], timeout: Duration) -> Result<(), StreamDeckError>;

    /// Check if transport is currently connected.
    fn is_connected(&self) -> bool;

    /// Close the transport.
    fn close(&self);
}

/// In-memory mock transport for unit tests, failure injection, and observable validation.
#[derive(Clone)]
pub struct MockTransport {
    input_queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
    written_reports: Arc<Mutex<Vec<Vec<u8>>>>,
    feature_reports: Arc<Mutex<Vec<Vec<u8>>>>,
    write_stall: Arc<Mutex<Option<Duration>>>,
    connected: Arc<AtomicBool>,
}

impl MockTransport {
    pub fn new() -> Self {
        Self {
            input_queue: Arc::new(Mutex::new(VecDeque::new())),
            written_reports: Arc::new(Mutex::new(Vec::new())),
            feature_reports: Arc::new(Mutex::new(Vec::new())),
            write_stall: Arc::new(Mutex::new(None)),
            connected: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Inject a raw HID input report to be read by the controller.
    pub fn inject_input_report(&self, report: &[u8]) {
        let mut queue = self.input_queue.lock().unwrap();
        queue.push_back(report.to_vec());
    }

    /// Inject a 511-byte standard hidraw report with given pressed key state.
    pub fn inject_button_states(&self, pressed: &[bool; STREAM_DECK_XL_KEY_COUNT]) {
        let mut report = vec![0u8; 511];
        report[0] = 0x01; // Report ID
        report[1] = 0x00;
        report[2] = 0x20; // 32 keys
        report[3] = 0x00;
        for (i, &p) in pressed.iter().enumerate() {
            if p {
                report[4 + i] = 0x01;
            }
        }
        self.inject_input_report(&report);
    }

    /// Inject a press or release event for a single key while keeping others unpressed.
    pub fn inject_key_action(&self, key: u8, pressed: bool) {
        let mut states = [false; STREAM_DECK_XL_KEY_COUNT];
        if pressed && (key as usize) < STREAM_DECK_XL_KEY_COUNT {
            states[key as usize] = true;
        }
        self.inject_button_states(&states);
    }

    /// Set an intentional write stall duration to simulate USB pipe delays or stalled hardware writes.
    pub fn set_write_stall(&self, stall: Option<Duration>) {
        let mut s = self.write_stall.lock().unwrap();
        *s = stall;
    }

    /// Simulate physical USB unplug.
    pub fn simulate_unplug(&self) {
        self.connected.store(false, Ordering::SeqCst);
    }

    /// Simulate physical USB replug.
    pub fn simulate_replug(&self) {
        self.connected.store(true, Ordering::SeqCst);
    }

    /// Retrieve all output reports written by the controller.
    pub fn get_written_reports(&self) -> Vec<Vec<u8>> {
        let reports = self.written_reports.lock().unwrap();
        reports.clone()
    }

    /// Clear recorded written reports.
    pub fn clear_written_reports(&self) {
        let mut reports = self.written_reports.lock().unwrap();
        reports.clear();
    }

    /// Retrieve all feature reports sent to the device.
    pub fn get_feature_reports(&self) -> Vec<Vec<u8>> {
        let reports = self.feature_reports.lock().unwrap();
        reports.clone()
    }
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for MockTransport {
    fn read(&self, buf: &mut [u8], timeout: Duration) -> Result<usize, StreamDeckError> {
        if !self.connected.load(Ordering::SeqCst) {
            return Err(StreamDeckError::NotConnected);
        }

        let start = std::time::Instant::now();
        loop {
            if !self.connected.load(Ordering::SeqCst) {
                return Err(StreamDeckError::NotConnected);
            }

            {
                let mut queue = self.input_queue.lock().unwrap();
                if let Some(report) = queue.pop_front() {
                    let len = report.len().min(buf.len());
                    buf[..len].copy_from_slice(&report[..len]);
                    return Ok(len);
                }
            }

            if start.elapsed() >= timeout {
                return Ok(0); // Timed out waiting for report
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn write(&self, buf: &[u8], timeout: Duration) -> Result<usize, StreamDeckError> {
        if !self.connected.load(Ordering::SeqCst) {
            return Err(StreamDeckError::NotConnected);
        }

        let stall = {
            let s = self.write_stall.lock().unwrap();
            *s
        };

        if let Some(stall_duration) = stall {
            if stall_duration > timeout {
                std::thread::sleep(timeout);
                return Err(StreamDeckError::Timeout("write stalled beyond deadline".into()));
            } else {
                std::thread::sleep(stall_duration);
            }
        }

        let mut reports = self.written_reports.lock().unwrap();
        reports.push(buf.to_vec());
        Ok(buf.len())
    }

    fn send_feature(&self, buf: &[u8], _timeout: Duration) -> Result<(), StreamDeckError> {
        if !self.connected.load(Ordering::SeqCst) {
            return Err(StreamDeckError::NotConnected);
        }
        let mut reports = self.feature_reports.lock().unwrap();
        reports.push(buf.to_vec());
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    fn close(&self) {
        self.connected.store(false, Ordering::SeqCst);
    }
}

/// Linux native hidraw transport opening `/dev/streamdeck_xl` or `/dev/hidraw*`.
pub struct HidrawTransport {
    #[cfg(target_os = "linux")]
    read_file: Mutex<Option<std::fs::File>>,
    #[cfg(target_os = "linux")]
    write_file: Mutex<Option<std::fs::File>>,
    #[allow(dead_code)]
    path: Option<std::path::PathBuf>,
    connected: AtomicBool,
}

impl HidrawTransport {
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "linux")]
            read_file: Mutex::new(None),
            #[cfg(target_os = "linux")]
            write_file: Mutex::new(None),
            path: None,
            connected: AtomicBool::new(false),
        }
    }

    /// Try to open the Stream Deck XL device from standard appliance paths.
    pub fn open_default() -> Result<Self, StreamDeckError> {
        let transport = Self::new();
        transport.reconnect()?;
        Ok(transport)
    }

    pub fn reconnect(&self) -> Result<(), StreamDeckError> {
        #[cfg(target_os = "linux")]
        {
            use std::fs::OpenOptions;
            use std::os::unix::fs::OpenOptionsExt;

            let candidates = [
                std::path::PathBuf::from("/dev/streamdeck_xl"),
                std::path::PathBuf::from("/dev/streamdeck0"),
            ];

            for candidate in &candidates {
                if candidate.exists() {
                    if let Ok(f1) = OpenOptions::new()
                        .read(true)
                        .custom_flags(libc::O_NONBLOCK)
                        .open(candidate)
                    {
                        if let Ok(f2) = OpenOptions::new()
                            .write(true)
                            .custom_flags(libc::O_NONBLOCK)
                            .open(candidate)
                        {
                            *self.read_file.lock().unwrap() = Some(f1);
                            *self.write_file.lock().unwrap() = Some(f2);
                            self.connected.store(true, Ordering::SeqCst);
                            return Ok(());
                        }
                    }
                }
            }

            // Fallback scan /sys/class/hidraw
            if let Ok(entries) = std::fs::read_dir("/sys/class/hidraw") {
                for entry in entries.flatten() {
                    let uevent_path = entry.path().join("device/uevent");
                    if let Ok(content) = std::fs::read_to_string(&uevent_path) {
                        let matches_vid = content.contains("0FD9") || content.contains("0fd9");
                        let matches_pid = content.contains("006C") || content.contains("006c")
                            || content.contains("008F") || content.contains("008f");
                        if matches_vid && matches_pid {
                            let dev_path = std::path::PathBuf::from("/dev").join(entry.file_name());
                            if let Ok(f1) = OpenOptions::new()
                                .read(true)
                                .custom_flags(libc::O_NONBLOCK)
                                .open(&dev_path)
                            {
                                if let Ok(f2) = OpenOptions::new()
                                    .write(true)
                                    .custom_flags(libc::O_NONBLOCK)
                                    .open(&dev_path)
                                {
                                    *self.read_file.lock().unwrap() = Some(f1);
                                    *self.write_file.lock().unwrap() = Some(f2);
                                    self.connected.store(true, Ordering::SeqCst);
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
            }
            Err(StreamDeckError::NotConnected)
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(StreamDeckError::Io("Native hidraw is only supported on Linux".into()))
        }
    }
}

impl Default for HidrawTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for HidrawTransport {
    fn read(&self, buf: &mut [u8], timeout: Duration) -> Result<usize, StreamDeckError> {
        #[cfg(target_os = "linux")]
        {
            use std::io::Read;
            use std::os::unix::io::AsRawFd;

            let mut lock = self.read_file.lock().unwrap();
            let file = match lock.as_mut() {
                Some(f) => f,
                None => return Err(StreamDeckError::NotConnected),
            };

            let raw_fd = file.as_raw_fd();
            let mut pfd = libc::pollfd {
                fd: raw_fd,
                events: libc::POLLIN,
                revents: 0,
            };

            let timeout_ms = timeout.as_millis() as i32;
            let ret = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
            if ret < 0 {
                return Err(StreamDeckError::Io("poll error".into()));
            } else if ret == 0 {
                return Ok(0); // Timeout
            }

            if (pfd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL)) != 0 {
                self.close();
                return Err(StreamDeckError::NotConnected);
            }

            match file.read(buf) {
                Ok(n) => Ok(n),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
                Err(e) => {
                    self.close();
                    Err(StreamDeckError::Io(e.to_string()))
                }
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (buf, timeout);
            Err(StreamDeckError::NotConnected)
        }
    }

    fn write(&self, buf: &[u8], _timeout: Duration) -> Result<usize, StreamDeckError> {
        #[cfg(target_os = "linux")]
        {
            use std::io::Write;
            let mut lock = self.write_file.lock().unwrap();
            let file = match lock.as_mut() {
                Some(f) => f,
                None => return Err(StreamDeckError::NotConnected),
            };

            match file.write(buf) {
                Ok(n) => Ok(n),
                Err(e) => {
                    self.close();
                    Err(StreamDeckError::Io(e.to_string()))
                }
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = buf;
            Err(StreamDeckError::NotConnected)
        }
    }

    fn send_feature(&self, buf: &[u8], _timeout: Duration) -> Result<(), StreamDeckError> {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::io::AsRawFd;
            let lock = self.write_file.lock().unwrap();
            let file = match lock.as_ref() {
                Some(f) => f,
                None => return Err(StreamDeckError::NotConnected),
            };

            let len = buf.len() as u32;
            let req = 0xC0004806u32 | (len << 16);
            let ret = unsafe { libc::ioctl(file.as_raw_fd(), req as libc::c_ulong, buf.as_ptr()) };
            if ret < 0 {
                return Err(StreamDeckError::Io("HIDIOCSFEATURE ioctl failed".into()));
            }
            Ok(())
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = buf;
            Err(StreamDeckError::NotConnected)
        }
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    fn close(&self) {
        self.connected.store(false, Ordering::SeqCst);
        #[cfg(target_os = "linux")]
        {
            *self.read_file.lock().unwrap() = None;
            *self.write_file.lock().unwrap() = None;
        }
    }
}
