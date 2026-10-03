use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub enum Rate {
    Sampling,
    Reset,
    NoProgress,
    Value(f64),
}
#[derive(Debug)]
pub struct Observation<T> {
    pub at: Instant,
    pub cost: Duration,
    pub data: Result<T, String>,
}
#[derive(Debug)]
pub struct Cpu {
    pub id: String,
    /// Busy percent: iowait counts as idle; guest is already included in user/nice.
    pub busy_percent: Rate,
}
#[derive(Debug, PartialEq)]
pub struct Memory {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    /// Cached + SReclaimable - Shmem, excluding Buffers.
    pub cache_bytes: u64,
    pub buffers_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
}
#[derive(Debug)]
pub struct Network {
    pub name: String,
    pub ifindex: Option<u64>,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_bytes_per_sec: Rate,
    pub tx_bytes_per_sec: Rate,
}
#[derive(Debug)]
pub struct Disk {
    pub id: String,
    pub name: String,
    /// physical, partition, virtual or unknown; never summed across layers.
    pub kind: &'static str,
    pub read_bytes: u64,
    pub written_bytes: u64,
    pub read_bytes_per_sec: Rate,
    pub written_bytes_per_sec: Rate,
}
#[derive(Debug)]
pub struct Filesystem {
    pub device: String,
    pub device_id: String,
    pub mount: String,
    pub fs_type: String,
    pub space: Result<Space, String>,
}
#[derive(Debug)]
pub struct Space {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub free_bytes: u64,
    pub available_bytes: u64,
}
#[derive(Debug)]
pub struct Snapshot {
    pub at: Instant,
    pub cpu: Observation<Vec<Cpu>>,
    pub memory: Observation<Memory>,
    pub load: Observation<[f64; 3]>,
    pub network: Observation<Vec<Network>>,
    pub disks: Observation<Vec<Disk>>,
}

#[derive(Debug)]
pub struct LiveFrame {
    pub snapshot: Snapshot,
    pub filesystems: std::sync::Arc<Observation<Vec<Filesystem>>>,
}

#[cfg(test)]
pub fn fixture(at: Instant) -> LiveFrame {
    use std::sync::Arc;
    fn obs<T>(at: Instant, data: T) -> Observation<T> {
        Observation {
            at,
            cost: Duration::ZERO,
            data: Ok(data),
        }
    }
    LiveFrame {
        snapshot: Snapshot {
            at,
            cpu: obs(
                at,
                vec![
                    Cpu {
                        id: "cpu".into(),
                        busy_percent: Rate::Value(40.),
                    },
                    Cpu {
                        id: "cpu0".into(),
                        busy_percent: Rate::Value(40.),
                    },
                ],
            ),
            memory: obs(
                at,
                Memory {
                    total_bytes: 8 << 30,
                    available_bytes: 6 << 30,
                    used_bytes: 2 << 30,
                    cache_bytes: 1 << 30,
                    buffers_bytes: 0,
                    swap_total_bytes: 2 << 30,
                    swap_used_bytes: 0,
                },
            ),
            load: obs(at, [0.1, 0.2, 0.3]),
            network: obs(
                at,
                vec![Network {
                    name: "eth0".into(),
                    ifindex: Some(2),
                    rx_bytes: 10000,
                    tx_bytes: 20000,
                    rx_bytes_per_sec: Rate::Value(1024.),
                    tx_bytes_per_sec: Rate::Value(2048.),
                }],
            ),
            disks: obs(
                at,
                vec![Disk {
                    id: "8:0".into(),
                    name: "sda".into(),
                    kind: "physical",
                    read_bytes: 30000,
                    written_bytes: 40000,
                    read_bytes_per_sec: Rate::Value(3072.),
                    written_bytes_per_sec: Rate::Value(4096.),
                }],
            ),
        },
        filesystems: Arc::new(obs(
            at,
            vec![Filesystem {
                device: "/dev/sda".into(),
                device_id: "8:0".into(),
                mount: "/".into(),
                fs_type: "ext4".into(),
                space: Ok(Space {
                    total_bytes: 100 << 30,
                    used_bytes: 30 << 30,
                    free_bytes: 70 << 30,
                    available_bytes: 65 << 30,
                }),
            }],
        )),
    }
}
