use crate::{collectors::LinuxCollector, model::LiveFrame};
use crossterm::event::{self, Event};
use std::{
    io,
    sync::{
        Arc, Condvar, Mutex,
        mpsc::{SyncSender, TrySendError},
    },
    thread,
    time::Duration,
};
pub enum Message {
    Input(Event),
    InputError(io::Error),
    Sample,
}
struct Control {
    paused: bool,
    stop: bool,
    epoch: u64,
}
pub struct Worker {
    control: Arc<(Mutex<Control>, Condvar)>,
    latest: Arc<Mutex<Option<LiveFrame>>>,
}
impl Worker {
    pub fn spawn(sender: SyncSender<Message>, interval: Duration, fs_interval: Duration) -> Self {
        let control = Arc::new((
            Mutex::new(Control {
                paused: false,
                stop: false,
                epoch: 0,
            }),
            Condvar::new(),
        ));
        let latest = Arc::new(Mutex::new(None));
        let worker_control = control.clone();
        let slot = latest.clone();
        thread::spawn(move || {
            let (lock, wake) = &*worker_control;
            let mut collector = LinuxCollector::new();
            collector.set_filesystem_interval(fs_interval);
            let mut epoch = 0;
            loop {
                let mut state = lock.lock().unwrap();
                while state.paused && !state.stop {
                    state = wake.wait(state).unwrap();
                }
                if state.stop {
                    break;
                }
                if state.epoch != epoch {
                    collector.reset_rates();
                    epoch = state.epoch;
                }
                drop(state);
                let snapshot = collector.sample();
                let filesystems = collector.filesystems.as_ref().unwrap().clone();
                let state = lock.lock().unwrap();
                if state.stop {
                    break;
                }
                if !state.paused
                    && epoch == state.epoch
                    && !publish(
                        &slot,
                        &sender,
                        LiveFrame {
                            snapshot,
                            filesystems,
                        },
                    )
                {
                    break;
                }
                // Condvar wakes immediately for pause/resume/exit. Never hold locks during collection.
                let _ = wake.wait_timeout(state, interval).unwrap();
            }
        });
        Self { control, latest }
    }
    pub fn pause(&self, paused: bool) {
        let (lock, wake) = &*self.control;
        let mut state = lock.lock().unwrap();
        if state.paused != paused {
            state.paused = paused;
            state.epoch = state.epoch.wrapping_add(1);
        }
        self.latest.lock().unwrap().take();
        wake.notify_one();
    }
    pub fn take(&self) -> Option<LiveFrame> {
        self.latest.lock().unwrap().take()
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let (lock, wake) = &*self.control;
        lock.lock().unwrap().stop = true;
        wake.notify_one();
    }
}
/// This blocking input reader has no polling timer. On process exit the OS reclaims it;
/// do not join a thread waiting for a key after q/Ctrl-C.
pub fn input(sender: SyncSender<Message>) {
    thread::spawn(move || {
        loop {
            let message = match event::read() {
                Ok(e) => Message::Input(e),
                Err(e) => Message::InputError(e),
            };
            let failed = matches!(message, Message::InputError(_));
            if sender.send(message).is_err() || failed {
                break;
            }
        }
    });
}

fn publish(
    slot: &Mutex<Option<LiveFrame>>,
    sender: &SyncSender<Message>,
    frame: LiveFrame,
) -> bool {
    *slot.lock().unwrap() = Some(frame);
    match sender.try_send(Message::Sample) {
        Ok(()) | Err(TrySendError::Full(_)) => true,
        Err(TrySendError::Disconnected(_)) => false,
    }
}
pub struct HardwareWorker {
    control: Arc<(Mutex<Control>, Condvar)>,
    latest: Arc<Mutex<Option<crate::hardware::HardwareFrame>>>,
}
impl HardwareWorker {
    pub fn spawn(
        sender: SyncSender<Message>,
        interval: Duration,
        root: std::path::PathBuf,
        disable_nvml: bool,
    ) -> Self {
        let control = Arc::new((
            Mutex::new(Control {
                paused: false,
                stop: false,
                epoch: 0,
            }),
            Condvar::new(),
        ));
        let latest = Arc::new(Mutex::new(None));
        let worker_control = control.clone();
        let slot = latest.clone();
        thread::spawn(move || {
            let mut collector = crate::hardware::Collector::new(root, disable_nvml);
            let (lock, wake) = &*worker_control;
            loop {
                let mut state = lock.lock().unwrap();
                while state.paused && !state.stop {
                    state = wake.wait(state).unwrap();
                }
                if state.stop {
                    break;
                }
                let epoch = state.epoch;
                drop(state);
                let frame = collector.sample();
                let state = lock.lock().unwrap();
                if state.stop {
                    break;
                }
                if !state.paused && state.epoch == epoch {
                    *slot.lock().unwrap() = Some(frame);
                    if matches!(
                        sender.try_send(Message::Sample),
                        Err(TrySendError::Disconnected(_))
                    ) {
                        break;
                    }
                }
                let _ = wake.wait_timeout(state, interval).unwrap();
            }
        });
        Self { control, latest }
    }
    pub fn pause(&self, paused: bool) {
        let (lock, wake) = &*self.control;
        let mut state = lock.lock().unwrap();
        if state.paused != paused {
            state.paused = paused;
            state.epoch = state.epoch.wrapping_add(1);
        }
        self.latest.lock().unwrap().take();
        wake.notify_one();
    }
    pub fn take(&self) -> Option<crate::hardware::HardwareFrame> {
        self.latest.lock().unwrap().take()
    }
}
impl Drop for HardwareWorker {
    fn drop(&mut self) {
        let (lock, wake) = &*self.control;
        lock.lock().unwrap().stop = true;
        wake.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_notification_queue_keeps_only_latest_snapshot() {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let slot = Mutex::new(None);
        let start = std::time::Instant::now();
        for i in 0..100 {
            assert!(publish(
                &slot,
                &tx,
                crate::model::fixture(start + Duration::from_millis(i))
            ));
        }
        assert_eq!(
            slot.lock().unwrap().take().unwrap().snapshot.at,
            start + Duration::from_millis(99)
        );
        assert!(matches!(rx.try_recv(), Ok(Message::Sample)));
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn pause_and_resume_prime_rate_baselines() {
        let (tx, _rx) = std::sync::mpsc::sync_channel(32);
        let worker = Worker::spawn(tx, Duration::from_secs(3600), Duration::from_secs(30));
        let wait = || {
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            loop {
                if let Some(frame) = worker.take() {
                    return frame;
                }
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        wait();
        worker.pause(true);
        std::thread::sleep(Duration::from_millis(20));
        assert!(worker.take().is_none());
        worker.pause(false);
        let frame = wait();
        assert!(
            frame
                .snapshot
                .cpu
                .data
                .unwrap()
                .iter()
                .all(|c| c.busy_percent == crate::model::Rate::Sampling)
        );
    }
}
