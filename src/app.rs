use crate::{
    config::{Config, Theme},
    history::History,
    model::{Disk, Filesystem, LiveFrame, Network, Rate},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::{Duration, Instant};

pub const SECTIONS: [&str; 7] = [
    "CPU",
    "GPU",
    "Memory",
    "Storage",
    "Network",
    "Thermals",
    "Processes",
];
pub struct App {
    pub selected: usize,
    pub focused: bool,
    pub help: bool,
    pub paused: bool,
    pub light: bool,
    pub ascii: bool,
    pub no_color: bool,
    pub sample: u64,
    pub history: [u64; 60],
    pub demo: bool,
    pub live: Option<LiveFrame>,
    pub histories: [History; 6],
    pub hardware: Option<crate::hardware::HardwareFrame>,
    pub gpu_id: Option<String>,
    pub sensor_id: Option<String>,
    pub now: Instant,
    pub chart_origin: Instant,
    pub config: Config,
    pub hostname: String,
    pub interface: Option<String>,
    pub disk: Option<String>,
    pub mount: Option<String>,
}
impl App {
    pub fn new(light: bool, ascii: bool, no_color: bool) -> Self {
        let now = Instant::now();
        Self {
            demo: true,
            live: None,
            hardware: None,
            gpu_id: None,
            sensor_id: None,
            histories: std::array::from_fn(|_| History::new(120)),
            now,
            chart_origin: now,
            config: Config::default(),
            hostname: std::fs::read_to_string("/proc/sys/kernel/hostname")
                .unwrap_or_else(|_| "linux".into())
                .trim()
                .to_string(),
            interface: None,
            disk: None,
            mount: None,
            selected: 0,
            focused: false,
            help: false,
            paused: false,
            light,
            ascii,
            no_color,
            sample: 0,
            history: std::array::from_fn(|i| match i {
                20..=25 | 33..=38 | 49..=55 => 88 + i as u64 % 5,
                _ => 17 + (i as u64 * 3) % 11,
            }),
        }
    }
    pub fn configured(config: Config, demo: bool) -> Self {
        let mut app = Self::new(
            matches!(config.theme, Theme::Light),
            config.ascii,
            config.no_color,
        );
        app.histories = std::array::from_fn(|_| History::new(config.history));
        app.interface = config.network_interface.clone();
        app.disk = config.disk_device.clone();
        app.mount = config.mount.clone();
        app.config = config;
        app.demo = demo;
        app
    }
    pub fn stale(&self) -> bool {
        !self.demo
            && !self.paused
            && self.live.as_ref().is_some_and(|f| {
                self.now.saturating_duration_since(f.snapshot.at)
                    > Duration::from_millis(self.config.interval).mul_f64(3.0)
            })
    }
    pub fn network(&self) -> Option<&Network> {
        let entries = self.live.as_ref()?.snapshot.network.data.as_ref().ok()?;
        if let Some(name) = &self.interface {
            entries.iter().find(|n| &n.name == name)
        } else {
            entries
                .iter()
                .find(|n| n.name != "lo")
                .or_else(|| entries.first())
        }
    }
    pub fn filesystem(&self) -> Option<&Filesystem> {
        let entries = self.live.as_ref()?.filesystems.data.as_ref().ok()?;
        if let Some(mount) = &self.mount {
            entries.iter().find(|f| &f.mount == mount)
        } else {
            entries
                .iter()
                .find(|f| f.mount == "/")
                .or_else(|| entries.first())
        }
    }
    pub fn disk(&self) -> Option<&Disk> {
        let entries = self.live.as_ref()?.snapshot.disks.data.as_ref().ok()?;
        if let Some(name) = &self.disk {
            entries.iter().find(|d| &d.name == name || &d.id == name)
        } else {
            self.filesystem()
                .and_then(|fs| entries.iter().find(|d| d.id == fs.device_id))
                .or_else(|| {
                    entries.iter().find(|d| {
                        d.kind == "physical"
                            && !d.name.starts_with("sr")
                            && !d.name.starts_with("loop")
                    })
                })
                .or_else(|| entries.first())
        }
    }
    pub fn apply(&mut self, frame: LiveFrame) {
        self.now = Instant::now();
        self.live = Some(frame);
        self.sample = self.sample.wrapping_add(1);
        let f = self.live.as_ref().unwrap();
        let cpu = f
            .snapshot
            .cpu
            .data
            .as_ref()
            .ok()
            .and_then(|v| v.iter().find(|c| c.id == "cpu"))
            .and_then(|c| number(&c.busy_percent))
            .map(|v| [v, 0.]);
        let memory = f
            .snapshot
            .memory
            .data
            .as_ref()
            .ok()
            .map(|m| [m.used_bytes as f64, 0.]);
        self.histories[0].push("cpu", f.snapshot.cpu.at, cpu);
        self.histories[1].push("memory", f.snapshot.memory.at, memory);
        let disk = self.disk().map(|d| {
            (
                d.id.clone(),
                pair(&d.read_bytes_per_sec, &d.written_bytes_per_sec),
            )
        });
        let net = self.network().map(|n| {
            (
                format!("{}:{:?}", n.name, n.ifindex),
                pair(&n.rx_bytes_per_sec, &n.tx_bytes_per_sec),
            )
        });
        let f = self.live.as_ref().unwrap();
        let (id, values) = disk.unwrap_or_else(|| (self.histories[2].identity.clone(), None));
        self.histories[2].push(&id, f.snapshot.disks.at, values);
        let (id, values) = net.unwrap_or_else(|| (self.histories[3].identity.clone(), None));
        self.histories[3].push(&id, f.snapshot.network.at, values);
    }
    fn cycle(&mut self, forward: bool, mount: bool) {
        let Some(frame) = &self.live else {
            return;
        };
        let entries: Vec<String> = if mount {
            frame
                .filesystems
                .data
                .as_ref()
                .map(|v| v.iter().map(|f| f.mount.clone()).collect())
                .unwrap_or_default()
        } else if self.selected == 4 {
            frame
                .snapshot
                .network
                .data
                .as_ref()
                .map(|v| v.iter().map(|n| n.name.clone()).collect())
                .unwrap_or_default()
        } else if self.selected == 3 {
            frame
                .snapshot
                .disks
                .data
                .as_ref()
                .map(|v| v.iter().map(|d| d.id.clone()).collect())
                .unwrap_or_default()
        } else {
            return;
        };
        if entries.is_empty() {
            return;
        }
        let current = if mount {
            self.filesystem().map(|f| f.mount.clone())
        } else if self.selected == 4 {
            self.network().map(|n| n.name.clone())
        } else {
            self.disk().map(|d| d.id.clone())
        };
        let index = current
            .as_ref()
            .and_then(|id| entries.iter().position(|e| e == id))
            .unwrap_or(0);
        let next = if forward {
            (index + 1) % entries.len()
        } else {
            (index + entries.len() - 1) % entries.len()
        };
        if mount {
            self.mount = Some(entries[next].clone());
        } else if self.selected == 4 {
            self.interface = Some(entries[next].clone());
            self.histories[3].points.clear();
        } else {
            self.disk = Some(entries[next].clone());
            self.histories[2].points.clear();
        }
    }
    pub fn gpu(&self) -> Option<&crate::hardware::Gpu> {
        let gpus = &self.hardware.as_ref()?.gpus;
        if let Some(id) = &self.gpu_id {
            gpus.iter().find(|g| &g.id == id)
        } else {
            gpus.iter()
                .find(|g| {
                    matches!(
                        g.backend,
                        "NVML" | "AMDGPU sysfs" | "Intel Level Zero Sysman"
                    )
                })
                .or_else(|| gpus.first())
        }
    }
    pub fn sensor(&self) -> Option<&crate::hardware::Sensor> {
        let sensors = &self.hardware.as_ref()?.sensors;
        if let Some(id) = &self.sensor_id {
            sensors.iter().find(|s| &s.id == id)
        } else {
            sensors
                .iter()
                .find(|s| s.kind == "CPU")
                .or_else(|| sensors.first())
        }
    }
    pub fn hardware_stale(&self) -> bool {
        !self.paused
            && self.hardware.as_ref().is_some_and(|f| {
                self.now.saturating_duration_since(f.at)
                    > Duration::from_millis(self.config.hardware_interval) * 3
            })
    }
    pub fn apply_hardware(&mut self, frame: crate::hardware::HardwareFrame) {
        self.now = Instant::now();
        self.hardware = Some(frame);
        let gpu = self
            .gpu()
            .map(|g| (g.id.clone(), g.utilization.as_ref().ok().map(|v| [*v, 0.])));
        let sensor = self
            .sensor()
            .map(|s| (s.id.clone(), s.celsius.as_ref().ok().map(|v| [*v, 0.])));
        let at = self.hardware.as_ref().unwrap().at;
        let (id, v) = gpu.unwrap_or_else(|| (self.histories[4].identity.clone(), None));
        self.histories[4].push(&id, at, v);
        let (id, v) = sensor.unwrap_or_else(|| (self.histories[5].identity.clone(), None));
        self.histories[5].push(&id, at, v);
    }
    fn cycle_hardware(&mut self, forward: bool) {
        let Some(f) = &self.hardware else {
            return;
        };
        let (ids, current) = if self.selected == 1 {
            (
                f.gpus.iter().map(|g| g.id.clone()).collect::<Vec<_>>(),
                self.gpu().map(|g| g.id.clone()),
            )
        } else if self.selected == 5 {
            (
                f.sensors.iter().map(|s| s.id.clone()).collect::<Vec<_>>(),
                self.sensor().map(|s| s.id.clone()),
            )
        } else {
            return;
        };
        if ids.is_empty() {
            return;
        }
        let i = current
            .and_then(|id| ids.iter().position(|v| *v == id))
            .unwrap_or(0);
        let next = if forward {
            (i + 1) % ids.len()
        } else {
            (i + ids.len() - 1) % ids.len()
        };
        if self.selected == 1 {
            self.gpu_id = Some(ids[next].clone());
            self.histories[4].points.clear();
        } else {
            self.sensor_id = Some(ids[next].clone());
            self.histories[5].points.clear();
        }
    }
    pub fn cpu(&self) -> u16 {
        (24 + self.sample.wrapping_mul(7) % 49) as u16
    }
    pub fn tick(&mut self) {
        self.sample = self.sample.wrapping_add(1);
        self.history.rotate_left(1);
        self.history[59] = self.cpu() as u64;
    }
    pub fn key(&mut self, key: KeyEvent) -> bool {
        if key.code == KeyCode::Char('q')
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            return true;
        }
        if self.help {
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('?')) {
                self.help = false;
            }
            return false;
        }
        match key.code {
            KeyCode::Tab | KeyCode::Right | KeyCode::Down => {
                self.selected = (self.selected + 1) % SECTIONS.len()
            }
            KeyCode::BackTab | KeyCode::Left | KeyCode::Up => {
                self.selected = (self.selected + SECTIONS.len() - 1) % SECTIONS.len()
            }
            KeyCode::Char(c @ '1'..='7') => self.selected = c as usize - '1' as usize,
            KeyCode::Enter => self.focused = !self.focused,
            KeyCode::Esc => self.focused = false,
            KeyCode::Char(' ') => self.paused = !self.paused,
            KeyCode::Char(']') => {
                self.cycle(true, false);
                self.cycle_hardware(true);
            }
            KeyCode::Char('[') => {
                self.cycle(false, false);
                self.cycle_hardware(false);
            }
            KeyCode::Char('f') if self.selected == 3 => self.cycle(true, true),
            KeyCode::Char('t') => self.light = !self.light,
            KeyCode::Char('a') => self.ascii = !self.ascii,
            KeyCode::Char('c') => self.no_color = !self.no_color,
            KeyCode::Char('?') => self.help = true,
            _ => {}
        }
        false
    }
}

pub fn number(rate: &Rate) -> Option<f64> {
    if let Rate::Value(v) = rate {
        Some(*v)
    } else {
        None
    }
}
fn pair(first: &Rate, second: &Rate) -> Option<[f64; 2]> {
    Some([number(first)?, number(second)?])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_and_help() {
        let mut app = App::new(false, false, false);
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        app.key(key(KeyCode::BackTab));
        assert_eq!(app.selected, SECTIONS.len() - 1);
        app.key(key(KeyCode::Char('2')));
        app.key(key(KeyCode::Enter));
        assert_eq!(app.selected, 1);
        assert!(app.focused);
        app.key(key(KeyCode::Char('?')));
        app.key(key(KeyCode::Char('3')));
        assert_eq!(app.selected, 1);
        app.key(key(KeyCode::Esc));
        assert!(!app.help);
        app.key(key(KeyCode::Esc));
        assert!(!app.focused);
        assert!(app.key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)));
    }
}
