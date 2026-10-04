//! Small runtime NVML ABI boundary. No link-time NVIDIA dependency.
use super::{Gpu, Sensor};
use std::{
    ffi::{CStr, c_char, c_int, c_uint, c_void},
    ptr,
};
type Device = *mut c_void;
type Status = c_int;
type Count = unsafe extern "C" fn(*mut c_uint) -> Status;
type Handle = unsafe extern "C" fn(c_uint, *mut Device) -> Status;
type StringQuery = unsafe extern "C" fn(Device, *mut c_char, c_uint) -> Status;
#[repr(C)]
#[derive(Default)]
struct Util {
    gpu: c_uint,
    memory: c_uint,
}
#[repr(C)]
#[derive(Default)]
struct Memory {
    total: u64,
    free: u64,
    used: u64,
}

pub struct Nvml {
    library: *mut c_void,
    shutdown: unsafe extern "C" fn() -> Status,
    count: Count,
    handle: Handle,
    name: Option<StringQuery>,
    uuid: Option<StringQuery>,
    utilization: Option<unsafe extern "C" fn(Device, *mut Util) -> Status>,
    memory: Option<unsafe extern "C" fn(Device, *mut Memory) -> Status>,
    temperature: Option<unsafe extern "C" fn(Device, c_uint, *mut c_uint) -> Status>,
    power: Option<unsafe extern "C" fn(Device, *mut c_uint) -> Status>,
    error: Option<unsafe extern "C" fn(Status) -> *const c_char>,
}
impl Nvml {
    pub fn load() -> Result<Self, String> {
        Self::load_name(c"libnvidia-ml.so.1")
    }
    fn load_name(name: &CStr) -> Result<Self, String> {
        // All pointers remain valid until shutdown, then dlclose. Signatures follow NVML's C ABI.
        unsafe {
            let library = libc::dlopen(name.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
            if library.is_null() {
                return Err("NVML library unavailable".into());
            }
            let required = (|| {
                let init: unsafe extern "C" fn() -> Status =
                    symbol(library, c"nvmlInit_v2").ok_or("NVML missing nvmlInit_v2")?;
                let shutdown =
                    symbol(library, c"nvmlShutdown").ok_or("NVML missing nvmlShutdown")?;
                let count =
                    symbol(library, c"nvmlDeviceGetCount_v2").ok_or("NVML missing device count")?;
                let handle = symbol(library, c"nvmlDeviceGetHandleByIndex_v2")
                    .ok_or("NVML missing device handle")?;
                let code = init();
                if code != 0 {
                    return Err(format!("NVML initialization failed (status {code})"));
                }
                Ok(Self {
                    library,
                    shutdown,
                    count,
                    handle,
                    name: symbol(library, c"nvmlDeviceGetName"),
                    uuid: symbol(library, c"nvmlDeviceGetUUID"),
                    utilization: symbol(library, c"nvmlDeviceGetUtilizationRates"),
                    memory: symbol(library, c"nvmlDeviceGetMemoryInfo"),
                    temperature: symbol(library, c"nvmlDeviceGetTemperature"),
                    power: symbol(library, c"nvmlDeviceGetPowerUsage"),
                    error: symbol(library, c"nvmlErrorString"),
                })
            })();
            if required.is_err() {
                libc::dlclose(library);
            }
            required
        }
    }
    fn check(&self, code: Status) -> Result<(), String> {
        if code == 0 {
            return Ok(());
        }
        let reason = self
            .error
            .and_then(|f| {
                // NVML owns this static, NUL-terminated error string.
                let p = unsafe { f(code) };
                (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| format!("NVML status {code}"));
        Err(reason)
    }
    fn text(&self, dev: Device, f: Option<StringQuery>) -> Result<String, String> {
        let f = f.ok_or("NVML query symbol unavailable")?;
        let mut buf = [0u8; 128];
        self.check(unsafe { f(dev, buf.as_mut_ptr().cast(), buf.len() as u32) })?;
        let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
        Ok(super::clean(&String::from_utf8_lossy(&buf[..end])))
    }
    pub fn sample(&self) -> Result<(Vec<Gpu>, Vec<Sensor>), String> {
        let mut count = 0;
        self.check(unsafe { (self.count)(&mut count) })?;
        if count > 128 {
            return Err("NVML returned excessive device count".into());
        }
        let mut gpus = Vec::new();
        let mut sensors = Vec::new();
        for i in 0..count {
            let mut dev = ptr::null_mut();
            let handle = self
                .check(unsafe { (self.handle)(i, &mut dev) })
                .and_then(|()| {
                    if dev.is_null() {
                        Err("NVML returned null device".into())
                    } else {
                        Ok(())
                    }
                });
            let mut gpu = Gpu::unavailable(
                format!("nvml:{i}"),
                format!("NVIDIA {i}"),
                "NVML",
                "device unavailable",
            );
            if let Err(e) = handle {
                gpu.utilization = Err(e.clone());
                gpu.memory = Err(e.clone());
                gpu.power_watts = Err(e);
                gpus.push(gpu);
                continue;
            }
            gpu.id = self.text(dev, self.uuid).unwrap_or(gpu.id);
            gpu.name = self.text(dev, self.name).unwrap_or(gpu.name);
            gpu.utilization = (|| {
                let f = self.utilization.ok_or("NVML utilization unavailable")?;
                let mut v = Util::default();
                self.check(unsafe { f(dev, &mut v) })?;
                if v.gpu > 100 {
                    return Err("NVML utilization outside 0..100".into());
                }
                Ok(v.gpu as f64)
            })();
            gpu.memory = (|| {
                let f = self.memory.ok_or("NVML VRAM unavailable")?;
                let mut v = Memory::default();
                self.check(unsafe { f(dev, &mut v) })?;
                if v.total == 0 || v.used > v.total {
                    return Err("NVML invalid VRAM totals".into());
                }
                Ok((v.used, v.total))
            })();
            gpu.power_watts = (|| {
                let f = self.power.ok_or("NVML power unavailable")?;
                let mut v = 0;
                self.check(unsafe { f(dev, &mut v) })?;
                Ok(v as f64 / 1000.)
            })();
            let value = (|| {
                let f = self.temperature.ok_or("NVML temperature unavailable")?;
                let mut v = 0;
                self.check(unsafe { f(dev, 0, &mut v) })?;
                super::temperature(v as f64)
            })();
            sensors.push(Sensor {
                id: format!("{}:temperature", gpu.id),
                device: gpu.id.clone(),
                label: "GPU core".into(),
                kind: "GPU",
                source: "NVML".into(),
                celsius: value,
                critical: None,
            });
            gpus.push(gpu);
        }
        Ok((gpus, sensors))
    }
}
// Private loader: each call supplies the exact documented function signature.
unsafe fn symbol<T: Copy>(library: *mut c_void, name: &CStr) -> Option<T> {
    let pointer = unsafe { libc::dlsym(library, name.as_ptr()) };
    if pointer.is_null() {
        None
    } else {
        assert_eq!(std::mem::size_of::<T>(), std::mem::size_of_val(&pointer));
        Some(unsafe { std::mem::transmute_copy(&pointer) })
    }
}
impl Drop for Nvml {
    fn drop(&mut self) {
        unsafe {
            (self.shutdown)();
            libc::dlclose(self.library);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_library_and_symbols_are_errors() {
        assert!(Nvml::load_name(c"/rtop-nonexistent/libnvidia-ml.so.1").is_err());
        assert!(Nvml::load_name(c"libc.so.6").is_err());
    }
}
