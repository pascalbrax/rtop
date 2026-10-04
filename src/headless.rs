use crate::{collectors::LinuxCollector, model::*};
use std::{
    hint::black_box,
    io, thread,
    time::{Duration, Instant},
};

fn value(rate: &Rate) -> String {
    match rate {
        Rate::Value(v) => format!("{v:.6}"),
        Rate::Sampling => "sampling".into(),
        Rate::Reset => "reset".into(),
        Rate::NoProgress => "no-progress".into(),
    }
}
fn clean(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}
pub fn collect(count: u32, interval: Duration, filesystem_interval: Duration) -> io::Result<()> {
    use std::io::Write;
    let mut collector = LinuxCollector::new();
    collector.set_filesystem_interval(filesystem_interval);
    let start = Instant::now();
    let mut deadline = start;
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    writeln!(
        out,
        "# rtop M2 real Linux metrics; bytes, bytes/s, percent; monotonic timestamps; no aggregates across devices"
    )?;
    for sample in 0..count {
        thread::sleep(deadline.saturating_duration_since(Instant::now()));
        let snapshot = collector.sample();
        writeln!(
            out,
            "sample\t{sample}\t{:.6}",
            snapshot.at.duration_since(start).as_secs_f64()
        )?;
        match &snapshot.cpu.data {
            Ok(cpus) => {
                for cpu in cpus {
                    writeln!(out, "cpu\t{}\t{}", cpu.id, value(&cpu.busy_percent))?;
                }
            }
            Err(e) => writeln!(out, "error\tcpu\t{}", clean(e))?,
        }
        match &snapshot.memory.data {
            Ok(m) => writeln!(
                out,
                "memory\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                m.total_bytes,
                m.available_bytes,
                m.used_bytes,
                m.cache_bytes,
                m.buffers_bytes,
                m.swap_total_bytes,
                m.swap_used_bytes
            )?,
            Err(e) => writeln!(out, "error\tmemory\t{}", clean(e))?,
        }
        match &snapshot.load.data {
            Ok(v) => writeln!(out, "load\t{}\t{}\t{}", v[0], v[1], v[2])?,
            Err(e) => writeln!(out, "error\tload\t{}", clean(e))?,
        }
        match &snapshot.network.data {
            Ok(devices) => {
                for n in devices {
                    writeln!(
                        out,
                        "network\t{}\t{}\t{}\t{}\t{}\t{}",
                        clean(&n.name),
                        n.ifindex
                            .map_or_else(|| "unknown".into(), |i| i.to_string()),
                        n.rx_bytes,
                        n.tx_bytes,
                        value(&n.rx_bytes_per_sec),
                        value(&n.tx_bytes_per_sec)
                    )?;
                }
            }
            Err(e) => writeln!(out, "error\tnetwork\t{}", clean(e))?,
        }
        match &snapshot.disks.data {
            Ok(devices) => {
                for d in devices {
                    writeln!(
                        out,
                        "disk\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                        d.id,
                        clean(&d.name),
                        d.kind,
                        d.read_bytes,
                        d.written_bytes,
                        value(&d.read_bytes_per_sec),
                        value(&d.written_bytes_per_sec)
                    )?;
                }
            }
            Err(e) => writeln!(out, "error\tdisks\t{}", clean(e))?,
        }
        if let Some(fs) = &collector.filesystems {
            let age = Instant::now().duration_since(fs.at).as_secs_f64();
            match &fs.data {
                Ok(mounts) => {
                    for f in mounts {
                        match &f.space {
                            Ok(s) => writeln!(
                                out,
                                "filesystem\t{}\t{}\t{}\t{}\t{}\t{}\t{}\tage={age:.3}s",
                                clean(&f.device),
                                clean(&f.mount),
                                f.fs_type,
                                s.total_bytes,
                                s.used_bytes,
                                s.free_bytes,
                                s.available_bytes
                            )?,
                            Err(e) => writeln!(
                                out,
                                "error\tfilesystem:{}\t{}",
                                clean(&f.mount),
                                clean(e)
                            )?,
                        }
                    }
                }
                Err(e) => writeln!(out, "error\tfilesystems\t{}", clean(e))?,
            }
        }
        out.flush()?;
        deadline += interval;
        if deadline < Instant::now() {
            deadline = Instant::now() + interval;
        }
    }
    Ok(())
}
fn resources() -> io::Result<libc::rusage> {
    let mut out = std::mem::MaybeUninit::uninit();
    // SAFETY: getrusage initializes the provided struct on a successful return.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, out.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { out.assume_init() })
}
fn cpu_seconds(r: &libc::rusage) -> f64 {
    r.ru_utime.tv_sec as f64
        + r.ru_stime.tv_sec as f64
        + (r.ru_utime.tv_usec + r.ru_stime.tv_usec) as f64 / 1e6
}
fn rss() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmRSS:")
                .and_then(|v| v.split_whitespace().next()?.parse().ok())
        })
}
fn sleep_until(deadline: Instant) {
    thread::sleep(deadline.saturating_duration_since(Instant::now()));
}
pub fn benchmark(
    runs: u32,
    warmup: Duration,
    duration: Duration,
    interval: Duration,
    filesystem_interval: Duration,
) -> io::Result<()> {
    let mut collector = LinuxCollector::new();
    collector.set_filesystem_interval(filesystem_interval);
    println!(
        "run,wall_s,cpu_percent_one_core,samples,timer_wakeups,missed_deadlines,voluntary_switches,involuntary_switches,rss_start_kib,rss_end_kib,max_rss_kib,collection_p50_us,collection_p95_us,collection_max_us,cpu_mean_us,memory_mean_us,load_mean_us,network_mean_us,disk_mean_us,fs_refreshes,fs_mean_us,errors"
    );
    for run in 1..=runs {
        eprintln!("run {run}/{runs}: warmup {}s", warmup.as_secs());
        let until = Instant::now() + warmup;
        let mut next = Instant::now();
        while Instant::now() < until {
            black_box(collector.sample());
            next += interval;
            sleep_until(next.min(until));
        }
        let rss_start = rss().unwrap_or(0);
        let before = resources()?;
        let start = Instant::now();
        let end = start + duration;
        let mut next = start;
        let mut costs = Vec::new();
        let mut sums = [0u128; 5];
        let mut missed = 0u64;
        let mut errors = 0u64;
        let mut fs_count = 0;
        let mut fs_sum = 0u128;
        let mut fs_at = collector.filesystems.as_ref().map(|f| f.at);
        eprintln!(
            "run {run}/{runs}: measuring {}s at {}ms",
            duration.as_secs(),
            interval.as_millis()
        );
        while Instant::now() < end {
            let collection_start = Instant::now();
            let s = collector.sample();
            costs.push(collection_start.elapsed().as_micros());
            let group_costs = [
                s.cpu.cost,
                s.memory.cost,
                s.load.cost,
                s.network.cost,
                s.disks.cost,
            ];
            for (i, cost) in group_costs.into_iter().enumerate() {
                sums[i] += cost.as_micros();
            }
            errors += u64::from(s.cpu.data.is_err())
                + u64::from(s.memory.data.is_err())
                + u64::from(s.load.data.is_err())
                + u64::from(s.network.data.is_err())
                + u64::from(s.disks.data.is_err());
            if let Some(fs) = &collector.filesystems
                && Some(fs.at) != fs_at
            {
                fs_count += 1;
                fs_sum += fs.cost.as_micros();
                fs_at = Some(fs.at);
                errors += match &fs.data {
                    Err(_) => 1,
                    Ok(mounts) => mounts.iter().filter(|f| f.space.is_err()).count() as u64,
                };
            }
            black_box(s);
            next += interval;
            if Instant::now() > next {
                missed += 1;
                next = Instant::now() + interval;
            }
            sleep_until(next.min(end));
        }
        let wall = start.elapsed().as_secs_f64();
        let after = resources()?;
        let rss_end = rss().unwrap_or(0);
        costs.sort_unstable();
        let n = costs.len();
        if n == 0 {
            return Err(io::Error::other("no benchmark samples"));
        }
        let cpu = (cpu_seconds(&after) - cpu_seconds(&before)) / wall * 100.0;
        println!(
            "{run},{wall:.6},{cpu:.6},{n},{n},{missed},{},{},{rss_start},{rss_end},{},{},{},{},{},{},{},{},{},{fs_count},{},{}",
            after.ru_nvcsw - before.ru_nvcsw,
            after.ru_nivcsw - before.ru_nivcsw,
            after.ru_maxrss,
            costs[n / 2],
            costs[((n - 1) * 95) / 100],
            costs[n - 1],
            sums[0] / n as u128,
            sums[1] / n as u128,
            sums[2] / n as u128,
            sums[3] / n as u128,
            sums[4] / n as u128,
            fs_sum.checked_div(fs_count).unwrap_or(0),
            errors
        );
        eprintln!("run {run}/{runs}: CPU {cpu:.4}% of one core; {n} samples; errors {errors}");
    }
    Ok(())
}

/// Isolated hardware cost, including cached discovery refreshes and missing backends.
pub fn hardware_benchmark(
    root: std::path::PathBuf,
    disable_nvml: bool,
    runs: u32,
    warmup: Duration,
    duration: Duration,
    interval: Duration,
) -> io::Result<()> {
    let mut collector = crate::hardware::Collector::new(root, disable_nvml);
    println!(
        "run,wall_s,cpu_percent_one_core,samples,rss_start_kib,rss_end_kib,collection_p95_us,gpu_discovery_mean_us,sensor_mean_us,gpu_devices,supported_gpus,sensors,sensor_errors"
    );
    for run in 1..=runs {
        eprintln!("hardware run {run}/{runs}: warmup {}s", warmup.as_secs());
        let until = Instant::now() + warmup;
        let mut next = Instant::now();
        while Instant::now() < until {
            black_box(collector.sample());
            next += interval;
            sleep_until(next.min(until));
        }
        let rss_start = rss().unwrap_or(0);
        let before = resources()?;
        let start = Instant::now();
        let end = start + duration;
        let mut next = start;
        let mut costs = Vec::new();
        let mut sums = [0u128; 2];
        let mut counts = [0usize; 3];
        let mut errors = 0usize;
        eprintln!(
            "hardware run {run}/{runs}: measuring {}s",
            duration.as_secs()
        );
        while Instant::now() < end {
            let at = Instant::now();
            let f = collector.sample();
            costs.push(at.elapsed().as_micros());
            sums[0] += f.gpu_cost.as_micros();
            sums[1] += f.sensor_cost.as_micros();
            counts = [
                f.gpus.len(),
                f.gpus
                    .iter()
                    .filter(|g| g.backend != "unsupported DRM")
                    .count(),
                f.sensors.len(),
            ];
            errors += f.sensors.iter().filter(|s| s.celsius.is_err()).count();
            black_box(f);
            next += interval;
            if next < Instant::now() {
                next = Instant::now() + interval;
            }
            sleep_until(next.min(end));
        }
        let wall = start.elapsed().as_secs_f64();
        let after = resources()?;
        let cpu = (cpu_seconds(&after) - cpu_seconds(&before)) / wall * 100.;
        costs.sort_unstable();
        let n = costs.len();
        if n == 0 {
            return Err(io::Error::other("no hardware benchmark samples"));
        }
        println!(
            "{run},{wall:.6},{cpu:.6},{n},{rss_start},{},{},{},{},{},{},{},{errors}",
            rss().unwrap_or(0),
            costs[((n - 1) * 95) / 100],
            sums[0] / n as u128,
            sums[1] / n as u128,
            counts[0],
            counts[1],
            counts[2]
        );
        eprintln!(
            "hardware run {run}/{runs}: CPU {cpu:.4}% of one core; sensors {}; errors {errors}",
            counts[2]
        );
    }
    Ok(())
}
