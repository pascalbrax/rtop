//! Read-only Level Zero Sysman ABI. Loaded only for Intel DRM devices.
use super::{Gpu, Sensor};
use std::{
    collections::BTreeMap,
    ffi::{CStr, c_void},
    ptr,
};
type Handle = *mut c_void;
type Status = u32;
type Enum = unsafe extern "C" fn(Handle, *mut u32, *mut Handle) -> Status;
type Drivers = unsafe extern "C" fn(*mut u32, *mut Handle) -> Status;
#[repr(C)]
#[derive(Default)]
struct Pci {
    stype: u32,
    next: Handle,
    address: [u32; 4],
    link_gen: i32,
    width: i32,
    bandwidth: i64,
    counters: [u8; 3],
}
#[repr(C)]
#[derive(Default)]
struct Engine {
    stype: u32,
    next: Handle,
    kind: u32,
    subdevice: u8,
    id: u32,
}
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Counters {
    active: u64,
    time: u64,
}
#[repr(C)]
#[derive(Default)]
struct Memory {
    stype: u32,
    next: Handle,
    kind: u32,
    subdevice: u8,
    id: u32,
    location: u32,
    physical: u64,
    bus_width: i32,
    channels: i32,
}
#[repr(C)]
#[derive(Default)]
struct MemoryState {
    stype: u32,
    next: Handle,
    health: u32,
    free: u64,
    size: u64,
}
#[repr(C)]
#[derive(Default)]
struct Temperature {
    stype: u32,
    next: Handle,
    kind: u32,
    subdevice: u8,
    id: u32,
    maximum: f64,
    critical: u8,
    threshold1: u8,
    threshold2: u8,
}

pub struct Intel {
    library: Handle,
    drivers: Drivers,
    devices: Enum,
    pci: unsafe extern "C" fn(Handle, *mut Pci) -> Status,
    engines: Option<Enum>,
    engine_properties: Option<unsafe extern "C" fn(Handle, *mut Engine) -> Status>,
    activity: Option<unsafe extern "C" fn(Handle, *mut Counters) -> Status>,
    memories: Option<Enum>,
    memory_properties: Option<unsafe extern "C" fn(Handle, *mut Memory) -> Status>,
    memory_state: Option<unsafe extern "C" fn(Handle, *mut MemoryState) -> Status>,
    temperatures: Option<Enum>,
    temperature_properties: Option<unsafe extern "C" fn(Handle, *mut Temperature) -> Status>,
    temperature_state: Option<unsafe extern "C" fn(Handle, *mut f64) -> Status>,
    previous: BTreeMap<String, (usize, Counters)>,
}
pub struct Reading {
    pub pci: String,
    pub gpu: Gpu,
    pub sensors: Vec<Sensor>,
}
impl Intel {
    pub fn load() -> Result<Self, String> {
        Self::load_name(c"libze_loader.so.1")
    }
    fn load_name(name: &CStr) -> Result<Self, String> {
        // Function types and repr(C) structures match the public zes_api.h ABI.
        unsafe {
            let library = libc::dlopen(
                name.as_ptr(),
                libc::RTLD_NOW | libc::RTLD_LOCAL | libc::RTLD_NODELETE,
            );
            if library.is_null() {
                return Err("Intel Level Zero loader unavailable (libze_loader.so.1)".into());
            }
            let result = (|| {
                let init: unsafe extern "C" fn(u32) -> Status = symbol(library, c"zesInit").ok_or(
                    "Intel Sysman zesInit unavailable; a newer Level Zero loader is required",
                )?;
                let drivers = symbol(library, c"zesDriverGet")
                    .ok_or("Intel Sysman driver enumeration unavailable")?;
                let devices = symbol(library, c"zesDeviceGet")
                    .ok_or("Intel Sysman device enumeration unavailable")?;
                let pci = symbol(library, c"zesDevicePciGetProperties")
                    .ok_or("Intel Sysman PCI identification unavailable")?;
                check(init(0))?;
                Ok(Self {
                    library,
                    drivers,
                    devices,
                    pci,
                    engines: symbol(library, c"zesDeviceEnumEngineGroups"),
                    engine_properties: symbol(library, c"zesEngineGetProperties"),
                    activity: symbol(library, c"zesEngineGetActivity"),
                    memories: symbol(library, c"zesDeviceEnumMemoryModules"),
                    memory_properties: symbol(library, c"zesMemoryGetProperties"),
                    memory_state: symbol(library, c"zesMemoryGetState"),
                    temperatures: symbol(library, c"zesDeviceEnumTemperatureSensors"),
                    temperature_properties: symbol(library, c"zesTemperatureGetProperties"),
                    temperature_state: symbol(library, c"zesTemperatureGetState"),
                    previous: BTreeMap::new(),
                })
            })();
            if result.is_err() {
                libc::dlclose(library);
            }
            result
        }
    }
    pub fn reset_rates(&mut self) {
        self.previous.clear();
    }
    pub fn sample(&mut self) -> Result<(Vec<Reading>, Vec<String>), String> {
        let drivers = enumerate(|count, out| unsafe { (self.drivers)(count, out) })?;
        let mut output = Vec::new();
        let mut diagnostics = Vec::new();
        let mut next = BTreeMap::new();
        for driver in drivers {
            let devices = match components(driver, Some(self.devices)) {
                Ok(v) => v,
                Err(e) => {
                    diagnostics.push(e);
                    continue;
                }
            };
            for dev in devices {
                let mut p = Pci {
                    stype: 2,
                    ..Default::default()
                };
                if let Err(e) = check(unsafe { (self.pci)(dev, &mut p) }) {
                    diagnostics.push(format!("Intel device PCI: {e}"));
                    continue;
                }
                let [domain, bus, device, function] = p.address;
                if domain > 0xffff || bus > 0xff || device > 31 || function > 7 {
                    diagnostics.push("Intel invalid PCI address".into());
                    continue;
                }
                let pci = format!("{domain:04x}:{bus:02x}:{device:02x}.{function}");
                if output.iter().any(|r: &Reading| r.pci == pci) {
                    diagnostics.push(format!("Intel duplicate PCI device {pci}"));
                    continue;
                }
                let mut gpu = Gpu::unavailable(
                    format!("pci:{pci}"),
                    format!("Intel GPU {pci}"),
                    "Intel Level Zero Sysman",
                    "query unavailable",
                );
                gpu.utilization = self.utilization(dev, &pci, &mut next);
                gpu.memory = self.memory(dev);
                gpu.power_watts = Err("Intel power not collected".into());
                let sensors = self.temperatures(dev, &gpu.id).unwrap_or_else(|e| {
                    vec![Sensor {
                        id: format!("{}:temperature", gpu.id),
                        device: gpu.id.clone(),
                        label: "Intel temperature".into(),
                        kind: "GPU",
                        source: "Level Zero Sysman".into(),
                        celsius: Err(e),
                        critical: None,
                    }]
                });
                output.push(Reading { pci, gpu, sensors });
            }
        }
        self.previous = next; // Device/query disappearance discards its old rate baseline.
        Ok((output, diagnostics))
    }
    fn utilization(
        &self,
        dev: Handle,
        pci: &str,
        next: &mut BTreeMap<String, (usize, Counters)>,
    ) -> Result<f64, String> {
        let properties = self
            .engine_properties
            .ok_or("Intel engine properties unavailable")?;
        let activity = self.activity.ok_or("Intel engine activity unavailable")?;
        for engine in components(dev, self.engines)? {
            let mut p = Engine {
                stype: 5,
                ..Default::default()
            };
            check(unsafe { properties(engine, &mut p) })?;
            // Only the whole-device ALL group has the requested total semantics.
            // Never sum overlapping render/compute/media groups or guess from a single engine.
            if p.kind != 0 || p.subdevice != 0 {
                continue;
            }
            let mut current = Counters::default();
            check(unsafe { activity(engine, &mut current) })?;
            next.insert(pci.into(), (engine as usize, current));
            let previous = self
                .previous
                .get(pci)
                .filter(|(old, _)| *old == engine as usize)
                .map(|(_, c)| *c);
            return delta(previous, current);
        }
        Err("Intel whole-device engine counter unavailable".into())
    }
    fn memory(&self, dev: Handle) -> Result<(u64, u64), String> {
        let properties = self
            .memory_properties
            .ok_or("Intel VRAM properties unavailable")?;
        let state = self.memory_state.ok_or("Intel VRAM state unavailable")?;
        let mut local = Vec::new();
        for memory in components(dev, self.memories)? {
            let mut p = Memory {
                stype: 0xb,
                ..Default::default()
            };
            check(unsafe { properties(memory, &mut p) })?;
            if p.location != 1 {
                continue;
            } // System/shared RAM is not discrete VRAM.
            let mut s = MemoryState {
                stype: 0x1e,
                ..Default::default()
            };
            check(unsafe { state(memory, &mut s) })?;
            // physicalSize is preferred; state.size is retained for older implementations.
            let total = if p.physical > 0 { p.physical } else { s.size };
            if total == 0 || s.free > total {
                return Err("Intel invalid VRAM totals".into());
            }
            local.push((p.subdevice, total - s.free, total));
        }
        if local.is_empty() {
            return Err("Intel device-local VRAM unavailable".into());
        }
        let has_root = local.iter().any(|(sub, _, _)| *sub == 0);
        let mut used = 0u64;
        let mut total = 0u64;
        for (sub, u, t) in local {
            if has_root && sub != 0 {
                continue;
            }
            used = used.checked_add(u).ok_or("Intel VRAM size overflow")?;
            total = total.checked_add(t).ok_or("Intel VRAM size overflow")?;
        }
        Ok((used, total))
    }
    fn temperatures(&self, dev: Handle, id: &str) -> Result<Vec<Sensor>, String> {
        let properties = self
            .temperature_properties
            .ok_or("Intel temperature properties unavailable")?;
        let state = self
            .temperature_state
            .ok_or("Intel temperature state unavailable")?;
        let mut output = Vec::new();
        for (i, sensor) in components(dev, self.temperatures)?.into_iter().enumerate() {
            let mut p = Temperature {
                stype: 0x14,
                ..Default::default()
            };
            check(unsafe { properties(sensor, &mut p) })?;
            let label = match p.kind {
                0 => "Device maximum",
                1 => "GPU maximum",
                2 => "Memory maximum",
                3 => "Device minimum",
                4 => "GPU minimum",
                5 => "Memory minimum",
                6 => "Board maximum",
                7 => "Board minimum",
                8 => "Voltage regulator maximum",
                9 => "Composite",
                10 => "Board sensor",
                11 => "Voltage regulator sensor",
                _ => "Unknown temperature domain",
            };
            let mut value = 0.;
            let celsius = check(unsafe { state(sensor, &mut value) })
                .and_then(|()| super::temperature(value));
            output.push(Sensor {
                id: format!("{id}:sysman:temp:{i}:{}:{}:{}", p.kind, p.subdevice, p.id),
                device: id.into(),
                label: format!(
                    "{label}{}",
                    if p.subdevice != 0 {
                        format!(" / tile {}", p.id)
                    } else {
                        String::new()
                    }
                ),
                kind: "GPU",
                source: "Level Zero Sysman".into(),
                celsius,
                critical: None,
            });
            // maxTemperature is a sensor reporting limit, not a thermal critical threshold.
        }
        if output.is_empty() {
            return Err("Intel temperature sensors unavailable".into());
        }
        Ok(output)
    }
}
fn components(dev: Handle, f: Option<Enum>) -> Result<Vec<Handle>, String> {
    let f = f.ok_or("Intel Sysman enumeration unavailable")?;
    enumerate(|count, out| unsafe { f(dev, count, out) })
}
fn enumerate(mut f: impl FnMut(*mut u32, *mut Handle) -> Status) -> Result<Vec<Handle>, String> {
    let mut count = 0;
    check(f(&mut count, ptr::null_mut()))?;
    if count > 128 {
        return Err("Intel Sysman excessive handle count".into());
    }
    if count == 0 {
        return Ok(Vec::new());
    }
    let mut handles = vec![ptr::null_mut(); count as usize];
    let capacity = count;
    check(f(&mut count, handles.as_mut_ptr()))?;
    if count > capacity {
        return Err("Intel Sysman enumeration changed; retry next sample".into());
    }
    handles.truncate(count as usize);
    let mut unique = std::collections::BTreeSet::new();
    if handles
        .iter()
        .any(|h| h.is_null() || !unique.insert(*h as usize))
    {
        return Err("Intel Sysman null or duplicate handle".into());
    }
    Ok(handles)
}
fn delta(previous: Option<Counters>, current: Counters) -> Result<f64, String> {
    let previous = previous.ok_or("collecting")?;
    if current.time < previous.time || current.active < previous.active {
        return Err("reset".into());
    }
    let elapsed = current.time - previous.time;
    if elapsed == 0 {
        return Err("no progress".into());
    }
    let value = (current.active - previous.active) as f64 / elapsed as f64 * 100.;
    if !(0. ..=100.).contains(&value) {
        return Err("Intel engine activity outside 0..100; counter not usable".into());
    }
    Ok(value)
}
fn check(code: Status) -> Result<(), String> {
    if code == 0 {
        return Ok(());
    }
    let reason = match code {
        0x70000001 => "device lost",
        0x70010000 => "insufficient permissions",
        0x70010001 => "not available",
        0x70020000 => "dependency unavailable",
        0x78000001 => "uninitialized",
        0x78000003 => "unsupported feature",
        _ => "query failed",
    };
    Err(format!("Intel Sysman {reason} (0x{code:08x})"))
}
unsafe fn symbol<T: Copy>(library: Handle, name: &CStr) -> Option<T> {
    let p = unsafe { libc::dlsym(library, name.as_ptr()) };
    if p.is_null() {
        None
    } else {
        assert_eq!(std::mem::size_of::<T>(), std::mem::size_of_val(&p));
        Some(unsafe { std::mem::transmute_copy(&p) })
    }
}
impl Drop for Intel {
    fn drop(&mut self) {
        // Sysman has no shutdown API. We own no external handles requiring destruction.
        unsafe {
            libc::dlclose(self.library);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_loader_and_counter_gaps() {
        assert!(Intel::load_name(c"/rtop-missing/libze_loader.so.1").is_err());
        assert!(Intel::load_name(c"libc.so.6").is_err());
        let c = |active, time| Counters { active, time };
        assert_eq!(delta(None, c(100, 1000)), Err("collecting".into()));
        assert_eq!(delta(Some(c(100, 1000)), c(600, 3000)), Ok(25.));
        assert_eq!(
            delta(Some(c(100, 1000)), c(100, 1000)),
            Err("no progress".into())
        );
        assert_eq!(delta(Some(c(100, 1000)), c(1, 3000)), Err("reset".into()));
        assert!(delta(Some(c(100, 1000)), c(5000, 3000)).is_err());
    }
    #[test]
    fn sysman_abi_layouts() {
        assert_eq!(std::mem::size_of::<Pci>(), 56);
        assert_eq!(std::mem::size_of::<Engine>(), 32);
        assert_eq!(std::mem::size_of::<Memory>(), 48);
        assert_eq!(std::mem::size_of::<MemoryState>(), 40);
        assert_eq!(std::mem::size_of::<Temperature>(), 48);
    }
}
