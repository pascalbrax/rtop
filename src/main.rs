mod app;
mod collectors;
mod config;
mod hardware;
mod headless;
mod history;
mod model;
mod ui;
mod worker;

use app::App;
use clap::Parser;
use crossterm::event::{Event, KeyEventKind};
use ratatui::{Terminal, backend::TestBackend};
use std::{
    io,
    time::{Duration, Instant},
};

#[derive(Parser)]
#[command(
    version,
    about = "Linux monitor · live CPU/RAM/disks/network; NVIDIA/AMD/Intel GPU and sensors; processes are demo"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Report hardware capabilities without initializing a terminal.
    #[arg(long)]
    doctor: bool,
    /// Diagnose a sysfs fixture or alternate mounted sysfs tree.
    #[arg(long, global = true, default_value = "/sys")]
    hardware_sysfs: std::path::PathBuf,
    /// Disable NVIDIA runtime library loading.
    #[arg(long, global = true)]
    disable_nvml: bool,
    /// Disable Intel Level Zero loading; keep DRM discovery and hwmon sensors.
    #[arg(long, global = true)]
    disable_intel: bool,
    #[arg(long, value_parser=clap::value_parser!(u64).range(1000..=60000))]
    hardware_interval: Option<u64>,

    #[arg(long, value_parser = clap::value_parser!(u64).range(100..=60000))]
    interval: Option<u64>,
    #[arg(long)]
    light: bool,
    #[arg(long, conflicts_with = "light")]
    theme: Option<config::Theme>,
    #[arg(long)]
    config: Option<std::path::PathBuf>,
    #[arg(long, value_parser=clap::value_parser!(u32).range(10..=3600))]
    history: Option<u32>,
    /// Run the original deterministic simulated dashboard.
    #[arg(long)]
    demo: bool,
    /// Capture real data with --preview (default previews use fixtures).
    #[arg(long, requires = "preview")]
    live_preview: bool,
    #[arg(long, value_parser=clap::value_parser!(u8).range(1..=7))]
    section: Option<u8>,
    #[arg(long, num_args=0..=1, default_missing_value="true")]
    ascii: Option<bool>,
    #[arg(long, num_args=0..=1, default_missing_value="true")]
    no_color: Option<bool>,
    /// Print a deterministic screen without entering interactive mode.
    #[arg(long, value_name = "WIDTHxHEIGHT")]
    preview: Option<String>,
    /// Export the preview as an SVG terminal capture.
    #[arg(long, requires = "preview")]
    svg: bool,
    /// Collect real Linux metrics as TSV without starting the TUI.
    #[arg(long, conflicts_with_all = ["preview", "benchmark", "benchmark_hardware"], value_parser = clap::value_parser!(u32).range(1..=3600))]
    collect: Option<u32>,
    /// Benchmark the base collectors without rendering.
    #[arg(long, group = "benchmark_mode", conflicts_with = "preview")]
    benchmark: bool,
    /// Measure hardware collectors separately, including unavailable backends.
    #[arg(long, group = "benchmark_mode", conflicts_with = "preview")]
    benchmark_hardware: bool,
    #[arg(long, default_value_t = 3, requires = "benchmark_mode", value_parser = clap::value_parser!(u32).range(1..=100))]
    runs: u32,
    #[arg(long, default_value_t = 30, requires = "benchmark_mode", value_parser = clap::value_parser!(u64).range(0..=3600))]
    warmup: u64,
    #[arg(long, default_value_t = 300, requires = "benchmark_mode", value_parser = clap::value_parser!(u64).range(1..=86400))]
    duration: u64,
}

#[derive(clap::Subcommand)]
enum Command {
    Doctor,
}
fn main() -> io::Result<()> {
    let cli = Cli::parse();
    let mut config = config::Config::load(cli.config.as_deref())?;
    if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
        config.no_color = true;
    }
    if let Some(interval) = cli.interval {
        config.interval = interval;
    }
    if let Some(history) = cli.history {
        config.history = history as usize;
    }
    if let Some(theme) = cli.theme {
        config.theme = theme;
    }
    if cli.light {
        config.theme = config::Theme::Light;
    }
    if let Some(ascii) = cli.ascii {
        config.ascii = ascii;
    }
    if let Some(no_color) = cli.no_color {
        config.no_color = no_color;
    }
    if let Some(interval) = cli.hardware_interval {
        config.hardware_interval = interval;
    }
    config.validate()?;
    if cli.doctor || matches!(cli.command, Some(Command::Doctor)) {
        let mut collector = hardware::Collector::new(cli.hardware_sysfs, cli.disable_nvml);
        collector.disable_intel(cli.disable_intel);
        return match hardware::doctor(&collector.sample()) {
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
            result => result,
        };
    }
    crossterm::style::force_color_output(!config.no_color);
    let interval = Duration::from_millis(config.interval);
    if let Some(count) = cli.collect {
        return headless::collect(
            count,
            interval,
            Duration::from_millis(config.filesystem_interval),
        );
    }
    if cli.benchmark_hardware {
        return headless::hardware_benchmark(
            cli.hardware_sysfs,
            cli.disable_nvml,
            cli.disable_intel,
            cli.runs,
            Duration::from_secs(cli.warmup),
            Duration::from_secs(cli.duration),
            Duration::from_millis(config.hardware_interval),
        );
    }
    if cli.benchmark {
        return headless::benchmark(
            cli.runs,
            Duration::from_secs(cli.warmup),
            Duration::from_secs(cli.duration),
            interval,
            Duration::from_millis(config.filesystem_interval),
        );
    }
    let demo = cli.demo || cli.preview.is_some() && !cli.live_preview;
    let mut app = App::configured(config.clone(), demo);
    if let Some(section) = cli.section {
        app.selected = section as usize - 1;
        app.focused = true;
    }
    if cli.live_preview {
        let mut collector = collectors::LinuxCollector::new();
        let mut hardware = hardware::Collector::new(cli.hardware_sysfs.clone(), cli.disable_nvml);
        hardware.disable_intel(cli.disable_intel);
        collector.sample();
        hardware.sample();
        std::thread::sleep(interval);
        let snapshot = collector.sample();
        app.apply_hardware(hardware.sample());
        app.apply(model::LiveFrame {
            snapshot,
            filesystems: collector.filesystems.as_ref().unwrap().clone(),
        });
    }
    if let Some(size) = cli.preview {
        let (w, h) = size
            .split_once('x')
            .and_then(|(w, h)| Some((w.parse::<u16>().ok()?, h.parse::<u16>().ok()?)))
            .filter(|(w, h)| *w > 0 && *h > 0 && *w <= 500 && *h <= 200)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "preview requires WIDTHxHEIGHT, max 500x200",
                )
            })?;
        let mut terminal = Terminal::new(TestBackend::new(w, h))?;
        terminal.draw(|frame| ui::draw(frame, &app))?;
        if cli.svg {
            println!("{}", ui::buffer_svg(terminal.backend().buffer()));
        } else {
            println!("{}", ui::buffer_text(terminal.backend().buffer()));
        }
        return Ok(());
    }
    // init installs the restoring panic hook; guard covers normal exit and errors.
    let mut terminal = ratatui::init();
    let _guard = Restore;
    let (tx, rx) = std::sync::mpsc::sync_channel(32);
    worker::input(tx.clone());
    let hardware_worker = if demo {
        None
    } else {
        Some(worker::HardwareWorker::spawn(
            tx.clone(),
            Duration::from_millis(config.hardware_interval),
            cli.hardware_sysfs,
            cli.disable_nvml,
            cli.disable_intel,
        ))
    };
    let worker = if demo {
        None
    } else {
        Some(worker::Worker::spawn(
            tx,
            interval,
            Duration::from_millis(config.filesystem_interval),
        ))
    };
    let mut dirty = true;
    let mut demo_next = Instant::now() + interval;
    loop {
        if dirty {
            terminal.draw(|frame| ui::draw(frame, &app))?;
            dirty = false;
        }
        let indefinite = app.paused || !demo && app.stale();
        let message = if indefinite {
            Some(
                rx.recv()
                    .map_err(|_| io::Error::other("event channel closed"))?,
            )
        } else {
            let until = if demo {
                demo_next
            } else {
                app.live
                    .as_ref()
                    .map_or(Instant::now() + interval * 3, |f| {
                        f.snapshot.at + interval * 3
                    })
            };
            match rx.recv_timeout(until.saturating_duration_since(Instant::now())) {
                Ok(message) => Some(message),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
                Err(_) => return Err(io::Error::other("event channel closed")),
            }
        };
        match message {
            Some(worker::Message::Input(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                let before = (
                    app.selected,
                    app.focused,
                    app.help,
                    app.paused,
                    app.light,
                    app.ascii,
                    app.no_color,
                    app.interface.clone(),
                    app.disk.clone(),
                    app.mount.clone(),
                    app.gpu_id.clone(),
                    app.sensor_id.clone(),
                );
                let was_paused = app.paused;
                if app.key(key) {
                    break;
                }
                let after = (
                    app.selected,
                    app.focused,
                    app.help,
                    app.paused,
                    app.light,
                    app.ascii,
                    app.no_color,
                    app.interface.clone(),
                    app.disk.clone(),
                    app.mount.clone(),
                    app.gpu_id.clone(),
                    app.sensor_id.clone(),
                );
                dirty |= before != after;
                if before.6 != app.no_color {
                    crossterm::style::force_color_output(!app.no_color);
                }
                if was_paused != app.paused {
                    if let Some(worker) = &worker {
                        worker.pause(app.paused);
                    }
                    if let Some(worker) = &hardware_worker {
                        worker.pause(app.paused);
                    }
                    demo_next = Instant::now() + interval;
                }
            }
            Some(worker::Message::Input(Event::Resize(..))) => dirty = true,
            Some(worker::Message::InputError(e)) => return Err(e),
            None if !app.paused => {
                if demo {
                    app.tick();
                    demo_next = Instant::now() + interval;
                } else {
                    app.now = Instant::now();
                }
                dirty = true;
            }
            _ => {}
        }
        if let Some(worker) = &worker
            && let Some(frame) = worker.take()
            && !app.paused
        {
            let initial = app.live.is_none();
            let was_stale = app.stale();
            let was_hardware_stale = app.hardware_stale();
            app.apply(frame);
            let size = terminal.size()?;
            dirty |= initial
                || was_stale
                || was_hardware_stale != app.hardware_stale()
                || !app.help
                    && ((!app.focused && size.width >= 100 && size.height >= 32)
                        || matches!(app.selected, 0 | 2 | 3 | 4));
        }
        if let Some(worker) = &hardware_worker
            && let Some(frame) = worker.take()
            && !app.paused
        {
            let was_stale = app.stale();
            app.apply_hardware(frame);
            dirty |= was_stale != app.stale();
            let size = terminal.size()?;
            let dashboard = !app.focused && size.width >= 100 && size.height >= 32;
            // Fold slower hardware updates into the next healthy base frame. Dedicated
            // views and stalled/slower base collection draw directly.
            let coalesce = dashboard
                && config.interval <= config.hardware_interval
                && app.live.is_some()
                && !app.stale();
            dirty |= !app.help
                && ((!coalesce && dashboard) || (!dashboard && matches!(app.selected, 1 | 5)));
        }
    }
    Ok(())
}
struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        ratatui::restore();
    }
}
