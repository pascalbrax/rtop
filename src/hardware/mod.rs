mod nvml;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
#[derive(Debug)]
pub struct Gpu {
    pub id: String,
    pub name: String,
    pub backend: &'static str,
    pub utilization: Result<f64, String>,
    /// Used and total VRAM in bytes; never substituted with system RAM.
    pub memory: Result<(u64, u64), String>,
    pub power_watts: Result<f64, String>,
}
impl Gpu {
    fn unavailable(id: String, name: String, backend: &'static str, reason: &str) -> Self {
        Self {
            id,
            name,
            backend,
            utilization: Err(reason.into()),
            memory: Err(reason.into()),
            power_watts: Err(reason.into()),
        }
    }
}
#[derive(Debug)]
pub struct Sensor {
    pub id: String,
    pub device: String,
    pub label: String,
    pub kind: &'static str,
    pub source: String,
    pub celsius: Result<f64, String>,
    pub critical: Option<f64>,
}
#[derive(Debug)]
pub struct HardwareFrame {
    pub at: Instant,
    pub gpu_cost: Duration,
    pub sensor_cost: Duration,
    pub gpus: Vec<Gpu>,
    pub sensors: Vec<Sensor>,
    pub diagnostics: Vec<String>,
}
struct Drm {
    id: String,
    name: String,
    path: PathBuf,
    amd: bool,
}
struct Hwmon {
    id: String,
    device: String,
    label: String,
    kind: &'static str,
    source: String,
    input: PathBuf,
    critical: PathBuf,
}
pub struct Collector {
    root: PathBuf,
    disable_nvml: bool,
    nvml: Option<nvml::Nvml>,
    discovered: Option<Instant>,
    drm: Vec<Drm>,
    sensors: Vec<Hwmon>,
    diagnostics: Vec<String>,
}
impl Collector {
    pub fn new(root: PathBuf, disable_nvml: bool) -> Self {
        Self {
            root,
            disable_nvml,
            nvml: None,
            discovered: None,
            drm: Vec::new(),
            sensors: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
    fn discover(&mut self) {
        self.drm.clear();
        self.sensors.clear();
        self.diagnostics.clear();
        if !self.disable_nvml && self.nvml.is_none() {
            match nvml::Nvml::load() {
                Ok(v) => self.nvml = Some(v),
                Err(e) => self.diagnostics.push(e),
            }
        } else if self.disable_nvml {
            self.diagnostics.push("NVML disabled".into());
        }
        let cards = entries(&self.root.join("class/drm"), &mut self.diagnostics);
        for card in cards {
            let name = file_name(&card);
            if !numbered(&name, "card") {
                continue;
            }
            let path = card.join("device");
            let vendor = text(&path.join("vendor")).unwrap_or_else(|e| format!("unknown ({e})"));
            // NVIDIA devices are queried by NVML; leave an explicit DRM placeholder if NVML is absent.
            if vendor == "0x10de" && self.nvml.is_some() {
                continue;
            }
            let driver = fs::read_link(path.join("driver"))
                .ok()
                .map(|p| file_name(&p))
                .unwrap_or_default();
            let amd = vendor == "0x1002" && driver == "amdgpu";
            let identity = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            let id = clean(&identity.to_string_lossy());
            if self.drm.iter().any(|d| d.id == id) {
                continue;
            }
            let model = text(&path.join("product_name")).unwrap_or_else(|_| {
                format!(
                    "{} {}",
                    vendor,
                    text(&path.join("device")).unwrap_or_default()
                )
            });
            self.drm.push(Drm {
                id,
                name: format!("{name} {model}"),
                path,
                amd,
            });
        }
        for hw in entries(&self.root.join("class/hwmon"), &mut self.diagnostics) {
            let source = text(&hw.join("name")).unwrap_or_else(|_| file_name(&hw));
            let device = fs::canonicalize(hw.join("device"))
                .or_else(|_| fs::canonicalize(&hw))
                .unwrap_or_else(|_| hw.clone());
            let device_id = clean(&device.to_string_lossy());
            let kind = if matches!(source.as_str(), "coretemp" | "k10temp" | "zenpower") {
                "CPU"
            } else if matches!(source.as_str(), "amdgpu" | "nouveau" | "radeon")
                || self.drm.iter().any(|d| d.id == device_id)
            {
                "GPU"
            } else {
                "Other"
            };
            for input in entries(&hw, &mut self.diagnostics) {
                let name = file_name(&input);
                let Some(stem) = name.strip_suffix("_input") else {
                    continue;
                };
                if !numbered(stem, "temp") {
                    continue;
                }
                let id = format!("{device_id}:{source}:{stem}");
                if self.sensors.iter().any(|s| s.id == id) {
                    continue;
                }
                let label = text(&hw.join(format!("{stem}_label"))).unwrap_or_else(|_| stem.into());
                self.sensors.push(Hwmon {
                    id,
                    device: device_id.clone(),
                    label,
                    kind,
                    source: source.clone(),
                    input,
                    critical: hw.join(format!("{stem}_crit")),
                });
            }
        }
        if self.sensors.is_empty() {
            for zone in entries(&self.root.join("class/thermal"), &mut self.diagnostics) {
                if !numbered(&file_name(&zone), "thermal_zone") {
                    continue;
                }
                let id = clean(&zone.to_string_lossy());
                self.sensors.push(Hwmon {
                    id: id.clone(),
                    device: id,
                    label: text(&zone.join("type")).unwrap_or_else(|_| "unknown zone".into()),
                    kind: "Thermal zone",
                    source: "thermal".into(),
                    input: zone.join("temp"),
                    critical: zone.join("unavailable_crit"),
                });
            }
        }
        self.discovered = Some(Instant::now());
    }
    pub fn sample(&mut self) -> HardwareFrame {
        let start = Instant::now();
        if self
            .discovered
            .is_none_or(|at| start.duration_since(at) >= Duration::from_secs(30))
        {
            self.discover();
        }
        let mut diagnostics = self.diagnostics.clone();
        let mut gpus = Vec::new();
        let mut sensors = Vec::new();
        if let Some(nvml) = &self.nvml {
            match nvml.sample() {
                Ok((g, s)) => {
                    gpus.extend(g);
                    sensors.extend(s);
                }
                Err(e) => {
                    diagnostics.push(e.clone());
                    self.diagnostics.push(e);
                    self.nvml = None;
                }
            }
        }
        for d in &self.drm {
            let mut gpu = Gpu::unavailable(
                d.id.clone(),
                d.name.clone(),
                "unsupported DRM",
                "driver/backend unsupported or NVML unavailable",
            );
            if d.amd {
                gpu.backend = "AMDGPU sysfs";
                gpu.utilization = numeric(&d.path.join("gpu_busy_percent")).and_then(|v| {
                    if (0. ..=100.).contains(&v) {
                        Ok(v)
                    } else {
                        Err("utilization outside 0..100".into())
                    }
                });
                gpu.memory = (|| {
                    let total = integer(&d.path.join("mem_info_vram_total"))?;
                    let used = integer(&d.path.join("mem_info_vram_used"))?;
                    if total == 0 || used > total {
                        return Err("invalid VRAM totals".into());
                    }
                    Ok((used, total))
                })();
                gpu.power_watts =
                    Err("power unavailable; hwmon power is not always GPU-only on APUs".into());
            }
            gpus.push(gpu);
        }
        let gpu_cost = start.elapsed();
        let sensor_start = Instant::now();
        for s in &self.sensors {
            sensors.push(Sensor {
                id: s.id.clone(),
                device: s.device.clone(),
                label: s.label.clone(),
                kind: s.kind,
                source: s.source.clone(),
                celsius: numeric(&s.input).and_then(|v| temperature(v / 1000.)),
                critical: numeric(&s.critical)
                    .ok()
                    .and_then(|v| temperature(v / 1000.).ok()),
            });
        }
        if gpus.is_empty() {
            diagnostics.push("No GPU devices available from supported backends".into());
        }
        if sensors.is_empty() {
            diagnostics.push("No temperature sensors available".into());
        }
        HardwareFrame {
            at: Instant::now(),
            gpu_cost,
            sensor_cost: sensor_start.elapsed(),
            gpus,
            sensors,
            diagnostics,
        }
    }
}
fn entries(path: &Path, diagnostics: &mut Vec<String>) -> Vec<PathBuf> {
    match fs::read_dir(path) {
        Ok(iter) => {
            let mut out = Vec::new();
            for entry in iter.take(512) {
                match entry {
                    Ok(e) => out.push(e.path()),
                    Err(e) => diagnostics.push(format!("{}: {e}", path.display())),
                }
            }
            out.sort();
            out
        }
        Err(e) => {
            diagnostics.push(format!("{}: {e}", path.display()));
            Vec::new()
        }
    }
}
fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}
fn numbered(value: &str, prefix: &str) -> bool {
    value
        .strip_prefix(prefix)
        .is_some_and(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()))
}
pub(super) fn clean(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(240).collect()
}
fn text(path: &Path) -> Result<String, String> {
    let mut buf = String::new();
    fs::File::open(path)
        .and_then(|f| f.take(4096).read_to_string(&mut buf))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(clean(buf.trim()))
}
fn numeric(path: &Path) -> Result<f64, String> {
    text(path)?
        .parse::<f64>()
        .map_err(|_| format!("{}: invalid numeric value", path.display()))
}
fn integer(path: &Path) -> Result<u64, String> {
    text(path)?
        .parse()
        .map_err(|_| format!("{}: invalid byte count", path.display()))
}
pub(super) fn temperature(v: f64) -> Result<f64, String> {
    if v.is_finite() && (-273.15..=1000.).contains(&v) {
        Ok(v)
    } else {
        Err("invalid temperature value".into())
    }
}
pub fn doctor(frame: &HardwareFrame) {
    println!("rtop hardware diagnostics (read-only)");
    for d in &frame.diagnostics {
        println!("INFO {d}");
    }
    for gpu in &frame.gpus {
        println!("GPU {} | {} | {}", gpu.id, gpu.name, gpu.backend);
        println!(
            "  utilization: {:?}\n  VRAM used/total bytes: {:?}\n  power W: {:?}",
            gpu.utilization, gpu.memory, gpu.power_watts
        );
    }
    for s in &frame.sensors {
        println!(
            "SENSOR {} | {} | {} | {} | {} | {:?} C | critical {:?}",
            s.id, s.device, s.kind, s.source, s.label, s.celsius, s.critical
        );
    }
    println!(
        "COST GPU/discovery {:.3} ms; sensors {:.3} ms",
        frame.gpu_cost.as_secs_f64() * 1000.,
        frame.sensor_cost.as_secs_f64() * 1000.
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_unsupported_amd_and_removed_sensors() {
        let root = std::env::temp_dir().join(format!("rtop-hardware-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let mut c = Collector::new(root.clone(), true);
        let f = c.sample();
        assert!(f.gpus.is_empty());
        assert!(f.sensors.is_empty());
        let put = |p: &str, v: &str| {
            let p = root.join(p);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, v).unwrap();
        };
        put("class/drm/card0/device/vendor", "0x102b");
        put("class/drm/card1/device/vendor", "0x1002");
        std::os::unix::fs::symlink(
            "/drivers/amdgpu",
            root.join("class/drm/card1/device/driver"),
        )
        .unwrap();
        put("class/drm/card1/device/gpu_busy_percent", "42");
        put("class/drm/card1/device/mem_info_vram_total", "8589934592");
        put("class/drm/card1/device/mem_info_vram_used", "2147483648");
        put("class/hwmon/hwmon0/name", "coretemp");
        put("class/hwmon/hwmon0/temp1_input", "47500");
        put("class/hwmon/hwmon0/temp1_label", "Package id 0");
        c.discovered = None;
        let f = c.sample();
        assert_eq!(f.gpus.len(), 2);
        assert!(f.gpus[0].utilization.is_err());
        assert_eq!(f.gpus[1].utilization, Ok(42.));
        assert_eq!(f.gpus[1].memory, Ok((2 << 30, 8 << 30)));
        assert_eq!(f.sensors[0].kind, "CPU");
        assert_eq!(f.sensors[0].celsius, Ok(47.5));
        put("class/drm/card1/device/gpu_busy_percent", "NaN");
        assert!(c.sample().gpus[1].utilization.is_err());
        fs::remove_file(root.join("class/hwmon/hwmon0/temp1_input")).unwrap();
        assert!(c.sample().sensors[0].celsius.is_err());
        fs::remove_dir_all(root.join("class/hwmon")).unwrap();
        put("class/thermal/thermal_zone0/type", "acpitz");
        put("class/thermal/thermal_zone0/temp", "41000");
        c.discovered = None;
        let f = c.sample();
        assert_eq!(f.sensors[0].kind, "Thermal zone");
        fs::remove_dir_all(root).unwrap();
    }
}
