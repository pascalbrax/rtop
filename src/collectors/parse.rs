use crate::model::{Memory, Rate};
use std::collections::BTreeMap;

pub type CpuCounters = BTreeMap<String, [u64; 8]>;
pub type Counters = BTreeMap<String, (String, u64, u64)>;
fn number(s: Option<&str>) -> Result<u64, String> {
    s.ok_or("missing field")?
        .parse()
        .map_err(|_| "invalid counter".into())
}
pub fn cpu(input: &str) -> Result<CpuCounters, String> {
    let mut output = BTreeMap::new();
    for line in input.lines() {
        let mut fields = line.split_whitespace();
        let Some(id) = fields.next() else {
            continue;
        };
        if id != "cpu" && !(id.starts_with("cpu") && id[3..].parse::<u32>().is_ok()) {
            continue;
        }
        let mut values = [0; 8];
        for (i, v) in values.iter_mut().enumerate() {
            match fields.next() {
                Some(s) => *v = number(Some(s))?,
                None if i >= 4 => break,
                None => return Err("short CPU row".into()),
            }
        }
        output.insert(id.to_string(), values);
    }
    if !output.contains_key("cpu") {
        return Err("missing aggregate CPU".into());
    }
    Ok(output)
}
pub fn cpu_delta(before: &[u64; 8], after: &[u64; 8]) -> Rate {
    let mut delta = [0u64; 8];
    for i in 0..8 {
        // Linux documents that iowait may decrease. Clamp just that field.
        if i != 4 && after[i] < before[i] {
            return Rate::Reset;
        }
        delta[i] = after[i].saturating_sub(before[i]);
    }
    let Some(total) = delta.iter().try_fold(0u64, |sum, v| sum.checked_add(*v)) else {
        return Rate::Reset;
    };
    if total == 0 {
        return Rate::NoProgress;
    }
    let idle = delta[3].saturating_add(delta[4]);
    Rate::Value((total - idle) as f64 / total as f64 * 100.0)
}
pub fn memory(input: &str) -> Result<Memory, String> {
    let mut values = BTreeMap::new();
    for line in input.lines() {
        let mut fields = line.split_whitespace();
        let Some(key) = fields.next() else {
            continue;
        };
        if !matches!(
            key,
            "MemTotal:"
                | "MemAvailable:"
                | "Buffers:"
                | "Cached:"
                | "SReclaimable:"
                | "Shmem:"
                | "SwapTotal:"
                | "SwapFree:"
        ) {
            continue;
        }
        let value = number(fields.next())?
            .checked_mul(1024)
            .ok_or("memory overflow")?;
        if fields.next() != Some("kB") {
            return Err("unexpected memory unit".into());
        }
        values.insert(key, value);
    }
    let get = |key| {
        values
            .get(key)
            .copied()
            .ok_or_else(|| format!("missing {key}"))
    };
    let total = get("MemTotal:")?;
    let available = get("MemAvailable:")?;
    let swap = get("SwapTotal:")?;
    let swap_free = get("SwapFree:")?;
    if available > total || swap_free > swap {
        return Err("inconsistent memory counters".into());
    }
    Ok(Memory {
        total_bytes: total,
        available_bytes: available,
        used_bytes: total - available,
        cache_bytes: get("Cached:")?
            .saturating_add(get("SReclaimable:")?)
            .saturating_sub(get("Shmem:")?),
        buffers_bytes: get("Buffers:")?,
        swap_total_bytes: swap,
        swap_used_bytes: swap - swap_free,
    })
}
pub fn network(input: &str) -> Result<Counters, String> {
    let mut output = BTreeMap::new();
    for line in input.lines().skip(2) {
        let Some((name, data)) = line.rsplit_once(':') else {
            if line.trim().is_empty() {
                continue;
            }
            return Err("invalid network row".into());
        };
        let fields: Vec<_> = data.split_whitespace().collect();
        if fields.len() < 16 {
            return Err("short network row".into());
        }
        let name = name.trim().to_string();
        output.insert(
            name.clone(),
            (
                name,
                number(fields.first().copied())?,
                number(fields.get(8).copied())?,
            ),
        );
    }
    Ok(output)
}
pub fn disks(input: &str) -> Result<Counters, String> {
    let mut output = BTreeMap::new();
    for line in input.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.is_empty() {
            continue;
        }
        if fields.len() < 14 {
            return Err("short disk row".into());
        }
        let major = number(fields.first().copied())?;
        let minor = number(fields.get(1).copied())?;
        let read = number(fields.get(5).copied())?
            .checked_mul(512)
            .ok_or("disk overflow")?;
        let write = number(fields.get(9).copied())?
            .checked_mul(512)
            .ok_or("disk overflow")?;
        output.insert(format!("{major}:{minor}"), (fields[2].into(), read, write));
    }
    Ok(output)
}
pub fn load(input: &str) -> Result<[f64; 3], String> {
    let mut fields = input.split_whitespace();
    let mut out = [0.0f64; 3];
    for v in &mut out {
        *v = fields
            .next()
            .ok_or("missing load")?
            .parse()
            .map_err(|_| "invalid load")?;
        if !v.is_finite() || *v < 0.0 {
            return Err("invalid load".into());
        }
    }
    Ok(out)
}
pub fn rate(before: u64, after: u64, seconds: f64) -> Rate {
    if after < before {
        Rate::Reset
    } else if seconds <= 0.0 || !seconds.is_finite() {
        Rate::NoProgress
    } else {
        Rate::Value((after - before) as f64 / seconds)
    }
}
/// Decode mountinfo's octal escapes without assuming UTF-8 boundaries.
pub fn unescape(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\'
            && i + 3 < bytes.len()
            && bytes[i + 1..i + 4]
                .iter()
                .all(|c| (b'0'..=b'7').contains(c))
        {
            let v = (bytes[i + 1] - b'0') as u16 * 64
                + (bytes[i + 2] - b'0') as u16 * 8
                + (bytes[i + 3] - b'0') as u16;
            if v <= 255 {
                out.push(v as u8);
                i += 4;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cpu_guest_reset_and_iowait() {
        let a = cpu("cpu 100 10 20 200 30 0 0 0 60 10\ncpu3 1 0 0 3\n").unwrap();
        let b = cpu("cpu 150 10 30 220 50 0 0 0 90 10\n").unwrap();
        assert_eq!(cpu_delta(&a["cpu"], &b["cpu"]), Rate::Value(60.0));
        let mut c = b["cpu"];
        c[4] = 40;
        assert_eq!(cpu_delta(&b["cpu"], &c), Rate::NoProgress);
        c[0] = 1;
        assert_eq!(cpu_delta(&b["cpu"], &c), Rate::Reset);
        assert!(cpu("cpu 1 2 bad 4").is_err());
    }
    #[test]
    fn memory_definitions_and_malformed_units() {
        let fixture = "MemTotal: 1000 kB\nMemAvailable: 400 kB\nBuffers: 20 kB\nCached: 100 kB\nSReclaimable: 30 kB\nShmem: 10 kB\nSwapTotal: 200 kB\nSwapFree: 150 kB\n";
        let m = memory(fixture).unwrap();
        assert_eq!(m.used_bytes, 600 * 1024);
        assert_eq!(m.cache_bytes, 120 * 1024);
        assert_eq!(m.swap_used_bytes, 50 * 1024);
        assert!(memory(&fixture.replace("kB", "MB")).is_err());
        assert!(memory("MemTotal: 1 kB").is_err());
    }
    #[test]
    fn io_fields_units_and_irregular_time() {
        let n = network("header\nheader\n eth0: 1200 1 0 0 0 0 0 0 400 1 0 0 0 0 0 0\n").unwrap();
        assert_eq!(n["eth0"].1, 1200);
        assert_eq!(n["eth0"].2, 400);
        let d = disks("259 0 nvme0n1 1 2 3 4 5 6 7 8 0 9 10\n").unwrap();
        assert_eq!(d["259:0"].1, 1536);
        assert_eq!(d["259:0"].2, 3584);
        assert_eq!(rate(100, 350, 2.5), Rate::Value(100.0));
        assert_eq!(rate(350, 10, 1.0), Rate::Reset);
        assert_eq!(rate(1, 2, 0.0), Rate::NoProgress);
        assert!(disks("8 0 sda").is_err());
        assert!(network("h\nh\neth0: 1").is_err());
        assert_eq!(unescape("/disk\\040one/\\134name"), "/disk one/\\name");
        assert_eq!(unescape("/日本語"), "/日本語");
    }
}
