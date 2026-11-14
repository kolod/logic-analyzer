//! A transport that replays recorded traffic, so protocol code runs without hardware.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use la_core::Result;

use crate::driver::Transport;
use crate::error;

/// One step of a recording, consumed by one `read` call (a `ShortRead` modifies the next one).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportStep {
    /// These bytes arrive (split over several reads if the buffer is smaller).
    Bytes(Vec<u8>),
    /// The next [`Bytes`](Self::Bytes) step delivers at most this many bytes per read, as a
    /// slow or fragmenting link does.
    ShortRead(usize),
    /// Nothing arrives within the timeout.
    Timeout,
    /// The device is unplugged: `device.disconnected`.
    Disconnect,
    /// The transfer fails with this detail: `device.transport`.
    Error(String),
}

/// What a driver wrote to a [`RecordedTransport`]. Clone it before the transport is moved into
/// a driver.
#[derive(Debug, Clone, Default)]
pub struct TransportLog {
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl TransportLog {
    /// Every `write` call so far, in order.
    #[must_use]
    pub fn written(&self) -> Vec<Vec<u8>> {
        self.writes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn record(&self, data: &[u8]) {
        self.writes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(data.to_vec());
    }
}

/// A [`Transport`] that replays a recording and logs what is written to it. When the
/// recording is exhausted every read fails with `device.transport`, which makes a recording
/// that is too short visible instead of hanging a test.
#[derive(Debug)]
pub struct RecordedTransport {
    steps: VecDeque<TransportStep>,
    cap: Option<usize>,
    log: TransportLog,
}

impl RecordedTransport {
    /// Creates a transport that replays `steps`.
    #[must_use]
    pub fn new(steps: Vec<TransportStep>) -> Self {
        Self {
            steps: steps.into(),
            cap: None,
            log: TransportLog::default(),
        }
    }

    /// The shared log of writes.
    #[must_use]
    pub fn log(&self) -> TransportLog {
        self.log.clone()
    }

    /// Every `write` call so far, in order.
    #[must_use]
    pub fn written(&self) -> Vec<Vec<u8>> {
        self.log.written()
    }
}

impl Transport for RecordedTransport {
    fn read(&mut self, buf: &mut [u8], _timeout: Duration) -> Result<usize> {
        loop {
            match self.steps.pop_front() {
                None => {
                    return Err(error::transport(
                        "the recording ended before the capture did (add steps to the recording)",
                    ));
                }
                Some(TransportStep::ShortRead(n)) => self.cap = Some(n.max(1)),
                Some(TransportStep::Bytes(mut data)) => {
                    let n = data
                        .len()
                        .min(buf.len())
                        .min(self.cap.unwrap_or(usize::MAX));
                    let rest = data.split_off(n);
                    if let Some(target) = buf.get_mut(..n) {
                        target.copy_from_slice(&data);
                    }
                    if !rest.is_empty() {
                        self.steps.push_front(TransportStep::Bytes(rest));
                    } else {
                        self.cap = None;
                    }
                    return Ok(n);
                }
                Some(TransportStep::Timeout) => {
                    std::thread::sleep(Duration::from_millis(1));
                    return Ok(0);
                }
                Some(TransportStep::Disconnect) => {
                    return Err(error::disconnected("the recorded device was unplugged"));
                }
                Some(TransportStep::Error(detail)) => return Err(error::transport(detail)),
            }
        }
    }

    fn write(&mut self, data: &[u8], _timeout: Duration) -> Result<usize> {
        self.log.record(data);
        Ok(data.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: Duration = Duration::from_millis(1);

    #[test]
    fn bytes_are_split_to_the_buffer_and_a_short_read_caps_the_next_step() {
        let mut t = RecordedTransport::new(vec![
            TransportStep::ShortRead(2),
            TransportStep::Bytes(vec![1, 2, 3, 4, 5]),
            TransportStep::Bytes(vec![9, 9, 9]),
        ]);
        let mut buf = [0_u8; 8];
        assert_eq!(t.read(&mut buf, T).unwrap(), 2);
        assert_eq!(buf.get(..2), Some(&[1, 2][..]));
        assert_eq!(t.read(&mut buf, T).unwrap(), 2);
        assert_eq!(t.read(&mut buf, T).unwrap(), 1, "the rest of the step");
        assert_eq!(
            t.read(&mut buf, T).unwrap(),
            3,
            "the cap ended with its step"
        );
    }

    #[test]
    fn timeouts_disconnects_errors_and_exhaustion_are_typed() {
        let mut t = RecordedTransport::new(vec![
            TransportStep::Timeout,
            TransportStep::Error("boom".to_owned()),
            TransportStep::Disconnect,
        ]);
        let mut buf = [0_u8; 4];
        assert_eq!(t.read(&mut buf, T).unwrap(), 0);
        let e = t.read(&mut buf, T).unwrap_err();
        assert_eq!((e.code(), e.detail()), (error::TRANSPORT, "boom"));
        assert_eq!(t.read(&mut buf, T).unwrap_err().code(), error::DISCONNECTED);
        assert_eq!(t.read(&mut buf, T).unwrap_err().code(), error::TRANSPORT);
    }

    #[test]
    fn writes_are_logged_in_order() {
        let mut t = RecordedTransport::new(vec![]);
        let log = t.log();
        t.write(b"ab", T).unwrap();
        t.write(b"c", T).unwrap();
        assert_eq!(log.written(), vec![b"ab".to_vec(), b"c".to_vec()]);
        assert_eq!(t.written(), log.written());
    }
}
