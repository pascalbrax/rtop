use serde::Deserialize;
use std::{
    io,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Default, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub interval: u64,
    pub filesystem_interval: u64,
    pub hardware_interval: u64,
    pub history: usize,
    pub theme: Theme,
    pub ascii: bool,
    pub no_color: bool,
    pub network_interface: Option<String>,
    pub disk_device: Option<String>,
    pub mount: Option<String>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            interval: 1000,
            filesystem_interval: 30000,
            hardware_interval: 2000,
            history: 120,
            theme: Theme::Dark,
            ascii: false,
            no_color: false,
            network_interface: None,
            disk_device: None,
            mount: None,
        }
    }
}
impl Config {
    pub fn load(explicit: Option<&Path>) -> io::Result<Self> {
        let path = explicit.map(Path::to_path_buf).or_else(|| {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
                .map(|base| base.join("rtop/config.toml"))
        });
        let config = if let Some(path) = path {
            match std::fs::read_to_string(&path) {
                Ok(text) => toml::from_str(&text).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("{}: {e}", path.display()),
                    )
                })?,
                Err(e) if explicit.is_none() && e.kind() == io::ErrorKind::NotFound => {
                    Self::default()
                }
                Err(e) => return Err(io::Error::new(e.kind(), format!("{}: {e}", path.display()))),
            }
        } else {
            Self::default()
        };
        Ok(config)
    }
    pub fn validate(&self) -> io::Result<()> {
        if !(100..=60000).contains(&self.interval) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "interval must be 100..60000 ms",
            ));
        }
        if !(1000..=3600000).contains(&self.filesystem_interval) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "filesystem_interval must be 1000..3600000 ms",
            ));
        }
        if !(1000..=60000).contains(&self.hardware_interval) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "hardware_interval must be 1000..60000 ms",
            ));
        }
        if !(10..=3600).contains(&self.history) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "history must be 10..3600 samples",
            ));
        }
        for selector in [&self.network_interface, &self.disk_device, &self.mount]
            .into_iter()
            .flatten()
        {
            if selector.is_empty() || selector.chars().any(char::is_control) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "device selectors must be nonempty and contain no control characters",
                ));
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_toml_and_bounds() {
        let c: Config = toml::from_str(
            "interval=2000\nhistory=120\ntheme='light'\nnetwork_interface='eth0'\nascii=true",
        )
        .unwrap();
        c.validate().unwrap();
        assert!(matches!(c.theme, Theme::Light));
        assert!(c.ascii);
        assert!(toml::from_str::<Config>("intervall=500").is_err());
        assert!(toml::from_str::<Config>("ascii='yes'").is_err());
        let mut c = Config {
            interval: 0,
            ..Config::default()
        };
        assert!(c.validate().is_err());
        c.interval = 1000;
        c.hardware_interval = 0;
        assert!(c.validate().is_err());
        c.hardware_interval = 2000;
        c.history = usize::MAX;
        assert!(c.validate().is_err());
    }
}
