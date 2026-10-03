pub mod parse;

use crate::model::*;
use std::{
    collections::BTreeMap,
    ffi::CString,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

type Previous = BTreeMap<String, (String, u64, u64)>;
pub struct LinuxCollector {
    proc: PathBuf,
    sys: PathBuf,
    buffer: String,
    previous_cpu: parse::CpuCounters,
    previous_net: Previous,
    previous_disk: Previous,
    net_at: Option<Instant>,
    disk_at: Option<Instant>,
    pub filesystems: Option<std::sync::Arc<Observation<Vec<Filesystem>>>>,
    fs_interval: Duration,
    kinds: BTreeMap<String, &'static str>,
    kinds_at: Option<Instant>,
}
impl LinuxCollector {
    pub fn new() -> Self {
        Self::with_roots("/proc".into(), "/sys".into())
    }
    pub fn with_roots(proc: PathBuf, sys: PathBuf) -> Self {
        Self {
            proc,
            sys,
            buffer: String::with_capacity(16384),
            previous_cpu: BTreeMap::new(),
            previous_net: BTreeMap::new(),
            previous_disk: BTreeMap::new(),
            net_at: None,
            disk_at: None,
            filesystems: None,
            fs_interval: Duration::from_secs(30),
            kinds: BTreeMap::new(),
            kinds_at: None,
        }
    }
    fn read(&mut self, name: &str) -> Result<&str, String> {
        self.buffer.clear();
        File::open(self.proc.join(name))
            .and_then(|mut f| f.read_to_string(&mut self.buffer))
            .map_err(|e| format!("{name}: {e}"))?;
        Ok(&self.buffer)
    }
    fn cpu(&mut self) -> Result<Vec<Cpu>, String> {
        let counters = parse::cpu(self.read("stat")?)?;
        let output = counters
            .iter()
            .map(|(id, after)| Cpu {
                id: id.clone(),
                busy_percent: self
                    .previous_cpu
                    .get(id)
                    .map_or(Rate::Sampling, |before| parse::cpu_delta(before, after)),
            })
            .collect();
        self.previous_cpu = counters;
        Ok(output)
    }
    fn network(&mut self) -> Result<Vec<Network>, String> {
        let counters = parse::network(self.read("net/dev")?)?;
        let now = Instant::now();
        let mut next = Previous::new();
        let output = counters
            .iter()
            .map(|(name, (_, rx, tx))| {
                let ifindex =
                    std::fs::read_to_string(self.sys.join("class/net").join(name).join("ifindex"))
                        .ok()
                        .and_then(|s| s.trim().parse::<u64>().ok());
                let identity = format!(
                    "{name}:{}",
                    ifindex.map_or_else(|| "unknown".into(), |v| v.to_string())
                );
                let (rx_rate, tx_rate) = deltas(
                    self.previous_net.get(&identity),
                    *rx,
                    *tx,
                    self.net_at.map(|t| now.duration_since(t)),
                );
                next.insert(identity, (name.clone(), *rx, *tx));
                Network {
                    name: name.clone(),
                    ifindex,
                    rx_bytes: *rx,
                    tx_bytes: *tx,
                    rx_bytes_per_sec: rx_rate,
                    tx_bytes_per_sec: tx_rate,
                }
            })
            .collect();
        self.previous_net = next;
        self.net_at = Some(now);
        Ok(output)
    }
    fn disks(&mut self) -> Result<Vec<Disk>, String> {
        let counters = parse::disks(self.read("diskstats")?)?;
        let now = Instant::now();
        if self
            .kinds_at
            .is_none_or(|t| now.duration_since(t) >= self.fs_interval)
        {
            self.kinds.clear();
            self.kinds_at = Some(now);
        }
        let output = counters
            .iter()
            .map(|(id, (name, read, write))| {
                let base = self.sys.join("dev/block").join(id);
                let kind = *self.kinds.entry(id.clone()).or_insert_with(|| {
                    if base.join("partition").exists() {
                        "partition"
                    } else if let Ok(path) = std::fs::canonicalize(&base) {
                        if path.components().any(|p| p.as_os_str() == "virtual") {
                            "virtual"
                        } else {
                            "physical"
                        }
                    } else {
                        "unknown"
                    }
                });
                let before = self.previous_disk.get(id).filter(|(old, _, _)| old == name);
                let (r, w) = deltas(
                    before,
                    *read,
                    *write,
                    self.disk_at.map(|t| now.duration_since(t)),
                );
                Disk {
                    id: id.clone(),
                    name: name.clone(),
                    kind,
                    read_bytes: *read,
                    written_bytes: *write,
                    read_bytes_per_sec: r,
                    written_bytes_per_sec: w,
                }
            })
            .collect();
        self.previous_disk = counters;
        self.disk_at = Some(now);
        Ok(output)
    }
    fn filesystems(&mut self) -> Result<Vec<Filesystem>, String> {
        let input = self.read("self/mountinfo")?;
        let mut unique = BTreeMap::new();
        for line in input.lines() {
            let (left, right) = line
                .split_once(" - ")
                .ok_or("invalid mountinfo separator")?;
            let fields: Vec<_> = left.split_whitespace().collect();
            let mut tail = right.split_whitespace();
            let fs_type = tail.next().ok_or("missing filesystem type")?;
            let device = tail.next().ok_or("missing filesystem source")?;
            if fields.len() < 6 {
                return Err("short mountinfo row".into());
            }
            // Never stat network/FUSE mounts on the sampling thread.
            if !matches!(
                fs_type,
                "ext2"
                    | "ext3"
                    | "ext4"
                    | "xfs"
                    | "btrfs"
                    | "zfs"
                    | "vfat"
                    | "exfat"
                    | "ntfs3"
                    | "overlay"
            ) {
                continue;
            }
            let key = format!("{}:{fs_type}", fields[2]);
            let mount = parse::unescape(fields[4]);
            unique.entry(key).or_insert((
                parse::unescape(device),
                fields[2].to_string(),
                mount,
                fs_type.to_string(),
            ));
        }
        Ok(unique
            .into_values()
            .map(|(device, device_id, mount, fs_type)| {
                let space = space(Path::new(&mount));
                Filesystem {
                    device,
                    device_id,
                    mount,
                    fs_type,
                    space,
                }
            })
            .collect())
    }
    pub fn set_filesystem_interval(&mut self, interval: Duration) {
        self.fs_interval = interval;
    }
    pub fn reset_rates(&mut self) {
        self.previous_cpu.clear();
        self.previous_net.clear();
        self.previous_disk.clear();
        self.net_at = None;
        self.disk_at = None;
    }
    pub fn sample(&mut self) -> Snapshot {
        let at = Instant::now();
        let cpu = observe(|| self.cpu());
        let memory = observe(|| parse::memory(self.read("meminfo")?));
        let load = observe(|| parse::load(self.read("loadavg")?));
        let network = observe(|| self.network());
        let disks = observe(|| self.disks());
        if self
            .filesystems
            .as_ref()
            .is_none_or(|f| at.saturating_duration_since(f.at) >= self.fs_interval)
        {
            self.filesystems = Some(std::sync::Arc::new(observe(|| self.filesystems())));
        }
        Snapshot {
            at,
            cpu,
            memory,
            load,
            network,
            disks,
        }
    }
}
fn observe<T>(f: impl FnOnce() -> Result<T, String>) -> Observation<T> {
    let start = Instant::now();
    let data = f();
    let at = Instant::now();
    Observation {
        at,
        cost: at.duration_since(start),
        data,
    }
}
fn deltas(
    before: Option<&(String, u64, u64)>,
    read: u64,
    write: u64,
    elapsed: Option<Duration>,
) -> (Rate, Rate) {
    match (before, elapsed) {
        (Some((_, r, w)), Some(dt)) => (
            parse::rate(*r, read, dt.as_secs_f64()),
            parse::rate(*w, write, dt.as_secs_f64()),
        ),
        _ => (Rate::Sampling, Rate::Sampling),
    }
}
pub fn space(path: &Path) -> Result<Space, String> {
    use std::os::unix::ffi::OsStrExt;
    let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| "NUL in mount path")?;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: path is NUL-terminated; statvfs writes the complete struct on success.
    if unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    // SAFETY: checked success before reading the initialized result.
    let stats = unsafe { stats.assume_init() };
    let bytes = |blocks: u64| {
        blocks
            .checked_mul(stats.f_frsize)
            .ok_or_else(|| "filesystem overflow".to_string())
    };
    let total = bytes(stats.f_blocks)?;
    let free = bytes(stats.f_bfree)?;
    let available = bytes(stats.f_bavail)?;
    if free > total || available > free {
        return Err("inconsistent filesystem space".into());
    }
    Ok(Space {
        total_bytes: total,
        used_bytes: total - free,
        free_bytes: free,
        available_bytes: available,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hotplug_error_recovery_and_filesystem_dedup() {
        let root = std::env::temp_dir().join(format!("rtop-collector-{}", std::process::id()));
        std::fs::create_dir_all(root.join("net")).unwrap();
        std::fs::create_dir_all(root.join("self")).unwrap();
        let write = |name: &str, content: &str| std::fs::write(root.join(name), content).unwrap();
        write("stat", "cpu 1 0 1 8\ncpu0 1 0 1 8\n");
        write(
            "meminfo",
            "MemTotal: 1000 kB\nMemAvailable: 400 kB\nBuffers: 20 kB\nCached: 100 kB\nSReclaimable: 30 kB\nShmem: 10 kB\nSwapTotal: 0 kB\nSwapFree: 0 kB\n",
        );
        write("loadavg", "0.1 0.2 0.3 1/3 4");
        let net = "h\nh\neth0: 100 0 0 0 0 0 0 0 200 0 0 0 0 0 0 0\n";
        write("net/dev", net);
        write("diskstats", "8 0 sda 1 0 10 0 1 0 20 0 0 0 0\n");
        write(
            "self/mountinfo",
            "1 0 8:0 / / rw - ext4 /dev/sda rw\n2 1 8:0 / /alias rw - ext4 /dev/sda rw\n3 1 0:1 / /remote rw - nfs host:/share rw\n",
        );
        let mut c = LinuxCollector::with_roots(root.clone(), root.join("sys"));
        let first = c.sample();
        assert_eq!(first.cpu.data.unwrap()[0].busy_percent, Rate::Sampling);
        assert_eq!(
            c.filesystems.as_ref().unwrap().data.as_ref().unwrap().len(),
            1
        );
        write("stat", "cpu malformed");
        assert!(c.sample().cpu.data.is_err());
        write("stat", "cpu 2 0 2 16\n");
        write("net/dev", "h\nh\n");
        write("diskstats", "");
        let removed = c.sample();
        assert!(removed.network.data.unwrap().is_empty());
        assert!(removed.disks.data.unwrap().is_empty());
        assert!(removed.cpu.data.is_ok());
        write("net/dev", net);
        write("diskstats", "8 0 sda 1 0 10 0 1 0 20 0 0 0 0\n");
        let returned = c.sample();
        assert_eq!(
            returned.network.data.unwrap()[0].rx_bytes_per_sec,
            Rate::Sampling
        );
        assert_eq!(
            returned.disks.data.unwrap()[0].read_bytes_per_sec,
            Rate::Sampling
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
