use crate::model::{Observation, Rate};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::{collections::HashMap, fs::File, io::Read, path::PathBuf, time::Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    pub pid: u32,
    pub start: u64,
}
#[derive(Debug)]
pub struct Process {
    pub id: Identity,
    pub name: String,
    pub cpu: Rate,
    pub rss: u64,
}
pub struct Frame {
    pub observation: Observation<Vec<Process>>,
    pub skipped: usize,
}
struct Stat {
    id: Identity,
    name: String,
    ticks: u64,
    rss: u64,
}
fn stat(input: &[u8], pid: u32, page_size: u64) -> Result<Stat, &'static str> {
    let start = input
        .iter()
        .position(|b| *b == b'(')
        .ok_or("missing comm")?;
    let end = input
        .iter()
        .rposition(|b| *b == b')')
        .ok_or("missing comm end")?;
    if end <= start
        || std::str::from_utf8(&input[..start])
            .ok()
            .and_then(|s| s.trim().parse().ok())
            != Some(pid)
    {
        return Err("PID mismatch");
    }
    let tail = std::str::from_utf8(&input[end + 1..]).map_err(|_| "invalid stat")?;
    let fields: Vec<_> = tail.split_ascii_whitespace().take(22).collect();
    let value = |i: usize| {
        fields
            .get(i)
            .ok_or("short stat")?
            .parse::<u64>()
            .map_err(|_| "invalid counter")
    };
    let ticks = value(11)?.checked_add(value(12)?).ok_or("ticks overflow")?;
    Ok(Stat {
        id: Identity {
            pid,
            start: value(19)?,
        },
        name: String::from_utf8_lossy(&input[start + 1..end])
            .chars()
            .map(|c| if c.is_control() { '�' } else { c })
            .collect(),
        ticks,
        rss: value(21)?.checked_mul(page_size).ok_or("RSS overflow")?,
    })
}

pub struct Collector {
    root: PathBuf,
    previous: HashMap<u32, (u64, u64, Instant)>,
    buffer: Vec<u8>,
    hz: u64,
    page_size: u64,
}
impl Collector {
    pub fn new(root: PathBuf) -> Self {
        // SAFETY: sysconf accepts these constants and does not use pointers.
        let (hz, page_size) = unsafe {
            (
                libc::sysconf(libc::_SC_CLK_TCK),
                libc::sysconf(libc::_SC_PAGESIZE),
            )
        };
        Self {
            root,
            previous: HashMap::new(),
            buffer: Vec::with_capacity(1024),
            hz: hz.max(0) as u64,
            page_size: page_size.max(0) as u64,
        }
    }
    pub fn reset(&mut self) {
        self.previous.clear();
    }
    pub fn sample(&mut self) -> Frame {
        self.sample_until(|| false)
    }
    pub fn sample_until(&mut self, cancelled: impl Fn() -> bool) -> Frame {
        let begin = Instant::now();
        let mut skipped = 0;
        let data = (|| {
            if self.hz == 0 || self.page_size == 0 {
                return Err("sysconf clock/page size unavailable".into());
            }
            let entries = std::fs::read_dir(&self.root)
                .map_err(|e| format!("{}: {e}", self.root.display()))?;
            let mut next = HashMap::with_capacity(self.previous.len());
            let mut processes = Vec::with_capacity(self.previous.len());
            for entry in entries {
                if cancelled() {
                    return Err("collection cancelled".into());
                }
                let Ok(entry) = entry else {
                    skipped += 1;
                    continue;
                };
                let Some(pid) = entry
                    .file_name()
                    .to_str()
                    .and_then(|s| s.parse::<u32>().ok())
                else {
                    continue;
                };
                self.buffer.clear();
                let result = File::open(entry.path().join("stat"))
                    .and_then(|file| file.take(8192).read_to_end(&mut self.buffer));
                let at = Instant::now();
                let Ok(parsed) = result
                    .map_err(|_| "unreadable stat")
                    .and_then(|_| stat(&self.buffer, pid, self.page_size))
                else {
                    skipped += 1;
                    continue;
                };
                let cpu = match self.previous.get(&pid) {
                    Some((start, ticks, before)) if *start == parsed.id.start => {
                        crate::collectors::parse::rate(
                            *ticks,
                            parsed.ticks,
                            at.saturating_duration_since(*before).as_secs_f64() * self.hz as f64
                                / 100.,
                        )
                    }
                    _ => Rate::Sampling,
                };
                next.insert(pid, (parsed.id.start, parsed.ticks, at));
                processes.push(Process {
                    id: parsed.id,
                    name: parsed.name,
                    cpu,
                    rss: parsed.rss,
                });
            }
            self.previous = next;
            Ok(processes)
        })();
        if data.is_err() {
            self.reset();
        }
        let at = Instant::now();
        Frame {
            observation: Observation {
                at,
                cost: at.duration_since(begin),
                data,
            },
            skipped,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    Cpu,
    Ram,
    Pid,
    Name,
}
impl Sort {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cpu => "CPU",
            Self::Ram => "RSS",
            Self::Pid => "PID",
            Self::Name => "Name",
        }
    }
}
pub struct View {
    pub frame: Option<Frame>,
    pub rows: Vec<usize>,
    pub selected: Option<Identity>,
    pub cursor: usize,
    pub filter: String,
    pub editing: bool,
    pub sort: Sort,
    pub descending: bool,
    pub revision: u64,
    selection_pinned: bool,
}
impl Default for View {
    fn default() -> Self {
        Self {
            frame: None,
            rows: Vec::new(),
            selected: None,
            cursor: 0,
            filter: String::new(),
            editing: false,
            sort: Sort::Cpu,
            descending: true,
            revision: 0,
            selection_pinned: false,
        }
    }
}
impl View {
    pub fn apply(&mut self, frame: Frame) {
        self.frame = Some(frame);
        self.rebuild();
    }
    fn rebuild(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.rows.clear();
        let Some(processes) = self
            .frame
            .as_ref()
            .and_then(|f| f.observation.data.as_ref().ok())
        else {
            return;
        };
        let filter = self.filter.to_lowercase();
        self.rows.extend(
            processes
                .iter()
                .enumerate()
                .filter(|(_, p)| {
                    filter.is_empty()
                        || p.name.to_lowercase().contains(&filter)
                        || p.id.pid.to_string().contains(&filter)
                })
                .map(|(i, _)| i),
        );
        self.rows.sort_unstable_by(|a, b| {
            let (a, b) = (&processes[*a], &processes[*b]);
            let order = match self.sort {
                Sort::Cpu => match (crate::app::number(&a.cpu), crate::app::number(&b.cpu)) {
                    (Some(a), Some(b)) => a.total_cmp(&b),
                    (None, Some(_)) => return std::cmp::Ordering::Greater,
                    (Some(_), None) => return std::cmp::Ordering::Less,
                    _ => std::cmp::Ordering::Equal,
                },
                Sort::Ram => a.rss.cmp(&b.rss),
                Sort::Pid => a.id.pid.cmp(&b.id.pid),
                Sort::Name => a.name.cmp(&b.name),
            };
            (if self.descending {
                order.reverse()
            } else {
                order
            })
            .then_with(|| a.id.pid.cmp(&b.id.pid))
        });
        self.cursor = if self.selection_pinned {
            self.selected
                .and_then(|id| self.rows.iter().position(|i| processes[*i].id == id))
                .unwrap_or(self.cursor)
                .min(self.rows.len().saturating_sub(1))
        } else {
            0
        };
        self.selected = self.rows.get(self.cursor).map(|i| processes[*i].id);
    }
    pub fn key(&mut self, key: KeyEvent) -> bool {
        if self.editing {
            match key.code {
                KeyCode::Esc => {
                    self.editing = false;
                    self.filter.clear();
                }
                KeyCode::Enter => self.editing = false,
                KeyCode::Backspace => {
                    self.filter.pop();
                }
                KeyCode::Char(c)
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                        && !c.is_control()
                        && self.filter.len() < 256 =>
                {
                    self.filter.push(c)
                }
                _ => return true,
            }
            self.rebuild();
            return true;
        }
        if matches!(
            key.code,
            KeyCode::Up
                | KeyCode::Down
                | KeyCode::PageUp
                | KeyCode::PageDown
                | KeyCode::Home
                | KeyCode::End
        ) {
            self.selection_pinned = true;
        }
        let last = self.rows.len().saturating_sub(1);
        match key.code {
            KeyCode::Char('/') => self.editing = true,
            KeyCode::Char('s') => {
                self.sort = match self.sort {
                    Sort::Cpu => Sort::Ram,
                    Sort::Ram => Sort::Pid,
                    Sort::Pid => Sort::Name,
                    Sort::Name => Sort::Cpu,
                };
                self.descending = matches!(self.sort, Sort::Cpu | Sort::Ram);
                self.rebuild();
            }
            KeyCode::Char('r') => {
                self.descending = !self.descending;
                self.rebuild();
            }
            KeyCode::Down => self.cursor = (self.cursor + 1).min(last),
            KeyCode::Up => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::PageDown => self.cursor = (self.cursor + 15).min(last),
            KeyCode::PageUp => self.cursor = self.cursor.saturating_sub(15),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = last,
            _ => return false,
        }
        if let Some(processes) = self
            .frame
            .as_ref()
            .and_then(|f| f.observation.data.as_ref().ok())
        {
            self.selected = self.rows.get(self.cursor).map(|i| processes[*i].id);
        }
        self.revision = self.revision.wrapping_add(1);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn input(pid: u32, name: &str, ticks: u64, start: u64, rss: u64) -> String {
        let mut fields = vec!["0".to_string(); 22];
        fields[0] = "R".into();
        fields[11] = ticks.to_string();
        fields[19] = start.to_string();
        fields[21] = rss.to_string();
        format!("{pid} ({name}) {}", fields.join(" "))
    }
    #[test]
    fn stat_names_rss_and_malformed_input() {
        let value = stat(input(12, "a ) (b\n", 400, 77, 5).as_bytes(), 12, 4096).unwrap();
        assert_eq!(value.name, "a ) (b�");
        assert_eq!(value.rss, 20480);
        assert_eq!(value.id.start, 77);
        assert!(stat(b"12 (bad) R 0", 12, 4096).is_err());
        assert!(stat(input(12, "valid", 1, 1, u64::MAX).as_bytes(), 12, 4096).is_err());
        assert!(stat(input(12, "valid", 1, 1, 1).as_bytes(), 13, 4096).is_err());
    }
    #[test]
    fn cpu_over_100_pid_reuse_exit_errors_and_cancellation() {
        let root = std::env::temp_dir().join(format!("rtop-processes-{}", std::process::id()));
        std::fs::create_dir_all(root.join("12")).unwrap();
        let write = |ticks, start| {
            std::fs::write(root.join("12/stat"), input(12, "worker", ticks, start, 10)).unwrap()
        };
        write(0, 1);
        let mut c = Collector::new(root.clone());
        c.hz = 100;
        c.page_size = 4096;
        assert_eq!(c.sample().observation.data.unwrap()[0].cpu, Rate::Sampling);
        c.previous
            .insert(12, (1, 0, Instant::now() - Duration::from_secs(2)));
        write(400, 1);
        let frame = c.sample();
        let cpu = crate::app::number(&frame.observation.data.unwrap()[0].cpu).unwrap();
        assert!((199. ..=201.).contains(&cpu));
        write(0, 2);
        assert_eq!(c.sample().observation.data.unwrap()[0].cpu, Rate::Sampling);
        std::fs::remove_file(root.join("12/stat")).unwrap();
        let frame = c.sample();
        assert_eq!(frame.skipped, 1);
        assert!(frame.observation.data.unwrap().is_empty());
        write(20, 2);
        assert_eq!(c.sample().observation.data.unwrap()[0].cpu, Rate::Sampling);
        assert!(c.sample_until(|| true).observation.data.is_err());
        assert!(c.previous.is_empty());
        std::fs::remove_dir_all(root).unwrap();
        assert!(c.sample().observation.data.is_err());
    }
    fn frame(values: &[(u32, u64, f64, u64)]) -> Frame {
        Frame {
            skipped: 0,
            observation: Observation {
                at: Instant::now(),
                cost: Duration::ZERO,
                data: Ok(values
                    .iter()
                    .map(|(pid, start, cpu, rss)| Process {
                        id: Identity {
                            pid: *pid,
                            start: *start,
                        },
                        name: format!("worker-{pid}"),
                        cpu: Rate::Value(*cpu),
                        rss: *rss,
                    })
                    .collect()),
            },
        }
    }
    #[test]
    fn stable_selection_sort_filter_and_pid_reuse() {
        let mut v = View::default();
        v.apply(frame(&[
            (1, 1, 50., 100),
            (2, 2, 20., 200),
            (3, 3, 10., 300),
        ]));
        let key = |c| KeyEvent::new(c, KeyModifiers::NONE);
        v.key(key(KeyCode::Down));
        assert_eq!(v.selected.unwrap().pid, 2);
        v.apply(frame(&[
            (3, 3, 100., 300),
            (2, 2, 200., 200),
            (1, 1, 300., 100),
        ]));
        assert_eq!(v.selected.unwrap().pid, 2);
        assert_eq!(v.cursor, 1);
        v.key(key(KeyCode::Char('s')));
        assert_eq!(v.sort, Sort::Ram);
        assert_eq!(v.selected.unwrap().pid, 2);
        v.key(key(KeyCode::Char('/')));
        v.key(key(KeyCode::Char('3')));
        assert_eq!(v.rows.len(), 1);
        assert_eq!(v.selected.unwrap().pid, 3);
        v.key(key(KeyCode::Esc));
        assert_eq!(v.rows.len(), 3);
        v.key(key(KeyCode::End));
        assert_eq!(v.cursor, 2);
        v.apply(frame(&[(1, 99, 10., 100)]));
        assert_eq!(v.selected, Some(Identity { pid: 1, start: 99 }));
        v.apply(frame(&[]));
        assert!(v.selected.is_none());
        assert!(v.rows.is_empty());
    }
}
