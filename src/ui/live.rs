use super::*;
use crate::{app::number, model::Rate};
use std::time::Duration;

fn bytes(v: u64) -> String {
    let (scale, unit) = if v >= 1 << 30 {
        (1u64 << 30, "GiB")
    } else if v >= 1 << 20 {
        (1u64 << 20, "MiB")
    } else if v >= 1 << 10 {
        (1u64 << 10, "KiB")
    } else {
        (1, "B")
    };
    format!("{:.1} {unit}", v as f64 / scale as f64)
}
fn rate(r: &Rate) -> String {
    match r {
        Rate::Value(v) => format!("{}/s", bytes(*v as u64)),
        Rate::Sampling => "collecting".into(),
        Rate::Reset => "reset".into(),
        Rate::NoProgress => "no progress".into(),
    }
}
fn cpu(r: &Rate) -> String {
    match r {
        Rate::Value(v) => format!("{v:.1}%"),
        Rate::Sampling => "collecting".into(),
        Rate::Reset => "reset".into(),
        Rate::NoProgress => "no progress".into(),
    }
}

pub(super) fn panel(
    frame: &mut Frame,
    area: Rect,
    index: usize,
    app: &App,
    p: &Palette,
    detail: bool,
) {
    let accent = p.sections[index];
    let Some(f) = &app.live else {
        frame.render_widget(
            Paragraph::new("Collecting Linux metrics...").style(Style::default().fg(p.muted)),
            area,
        );
        return;
    };
    let mut lines = Vec::new();
    let mut usage = None;
    let mut summary = "N/D".to_string();
    let history = match index {
        0 => {
            match &f.snapshot.cpu.data {
                Ok(cpus) => {
                    if let Some(total) = cpus.iter().find(|c| c.id == "cpu") {
                        summary = cpu(&total.busy_percent);
                        usage = number(&total.busy_percent);
                    }
                    lines.push(match &f.snapshot.load.data {
                        Ok(v) => format!("Load {:.2} / {:.2} / {:.2}", v[0], v[1], v[2]),
                        Err(e) => format!("Load N/D: {e}"),
                    });
                    lines.push(format!(
                        "{} logical cores / kernel view",
                        cpus.len().saturating_sub(1)
                    ));
                    let mut cores: Vec<_> = cpus.iter().filter(|c| c.id != "cpu").collect();
                    cores.sort_by_key(|c| {
                        c.id.trim_start_matches("cpu")
                            .parse::<u32>()
                            .unwrap_or(u32::MAX)
                    });
                    let per_row = (area.width as usize / 11).max(1);
                    for chunk in cores.chunks(per_row) {
                        lines.push(
                            chunk
                                .iter()
                                .map(|c| {
                                    format!(
                                        "{}:{:>5}",
                                        c.id.trim_start_matches("cpu"),
                                        cpu(&c.busy_percent)
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("  "),
                        );
                    }
                }
                Err(e) => lines.push(format!("CPU N/D: {e}")),
            }
            0
        }
        2 => {
            match &f.snapshot.memory.data {
                Ok(m) => {
                    let percent = m.used_bytes as f64 / m.total_bytes.max(1) as f64 * 100.;
                    summary = format!("{percent:.1}%");
                    usage = Some(percent);
                    lines.push(format!(
                        "Used {} / {}",
                        bytes(m.used_bytes),
                        bytes(m.total_bytes)
                    ));
                    lines.push(format!("Available {}", bytes(m.available_bytes)));
                    lines.push(format!(
                        "Cache {} / buffers {}",
                        bytes(m.cache_bytes),
                        bytes(m.buffers_bytes)
                    ));
                    lines.push(format!(
                        "Swap {} / {}",
                        bytes(m.swap_used_bytes),
                        bytes(m.swap_total_bytes)
                    ));
                }
                Err(e) => lines.push(format!("Memory N/D: {e}")),
            }
            1
        }
        3 => {
            if let Err(e) = &f.filesystems.data {
                lines.push(format!("Space N/D: {e}"));
            } else if let Some(fs) = app.filesystem() {
                match &fs.space {
                    Ok(space) => {
                        let percent =
                            space.used_bytes as f64 / space.total_bytes.max(1) as f64 * 100.;
                        usage = Some(percent);
                        summary = format!("{percent:.1}%");
                        lines.push(format!(
                            "{} {} / {}",
                            fs.mount,
                            bytes(space.used_bytes),
                            bytes(space.total_bytes)
                        ));
                    }
                    Err(e) => lines.push(format!("{} N/D: {e}", fs.mount)),
                }
            } else {
                lines.push("Space N/D: mount unavailable".into());
            }
            if let Err(e) = &f.snapshot.disks.data {
                lines.push(format!("I/O N/D: {e}"));
            } else if let Some(d) = app.disk() {
                lines.push(format!(
                    "{} R {} W {}",
                    d.name,
                    rate(&d.read_bytes_per_sec),
                    rate(&d.written_bytes_per_sec)
                ));
                lines.push(format!("{} ({}) / [ ] select disk", d.id, d.kind));
            } else {
                lines.push("I/O N/D: device unavailable".into());
            }
            lines.push(format!(
                "Space age {:.0}s / f select mount",
                app.now
                    .saturating_duration_since(f.filesystems.at)
                    .as_secs_f64()
            ));
            2
        }
        _ => {
            if let Err(e) = &f.snapshot.network.data {
                lines.push(format!("Network N/D: {e}"));
            } else if let Some(n) = app.network() {
                summary = format!("RX {}", rate(&n.rx_bytes_per_sec));
                lines.push(format!("{} TX {}", n.name, rate(&n.tx_bytes_per_sec)));
                lines.push(format!(
                    "Total RX {} / TX {}",
                    bytes(n.rx_bytes),
                    bytes(n.tx_bytes)
                ));
                lines.push(format!(
                    "ifindex {} / [ ] select interface",
                    n.ifindex.map_or_else(|| "N/D".into(), |v| v.to_string())
                ));
            } else {
                lines.push("Network N/D: interface unavailable".into());
            }
            3
        }
    };
    if app.stale() {
        summary = format!("STALE {summary}");
    }
    let stats = if detail {
        lines
            .len()
            .min(area.height.saturating_sub(5) as usize)
            .max(1)
    } else {
        2
    };
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(stats as u16),
        Constraint::Min(2),
    ])
    .split(area);
    if let Some(percent) = usage.filter(|_| !app.stale()) {
        let count = chunks[0].width.saturating_sub(summary.len() as u16 + 2) as usize / 2;
        let filled = (count as f64 * percent.clamp(0., 100.) / 100.) as usize;
        let glyph = if app.ascii || app.no_color {
            "# "
        } else {
            "▮ "
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(glyph.repeat(filled), Style::default().fg(accent).bold()),
                Span::styled(
                    if app.no_color {
                        ". ".repeat(count - filled)
                    } else {
                        glyph.repeat(count - filled)
                    },
                    Style::default().fg(blend(accent, p.surface, 0.22)),
                ),
                Span::styled(format!(" {summary}"), Style::default().fg(accent).bold()),
            ])),
            chunks[0],
        );
    } else {
        frame.render_widget(
            Paragraph::new(summary).style(Style::default().fg(accent).bold()),
            chunks[0],
        );
    }
    frame.render_widget(
        Paragraph::new(lines.into_iter().map(Line::from).collect::<Vec<_>>())
            .style(Style::default().fg(p.fg)),
        chunks[1],
    );
    chart(frame, chunks[2], history, app, p, accent);
}
fn chart(frame: &mut Frame, area: Rect, index: usize, app: &App, p: &Palette, accent: Color) {
    let history = &app.histories[index];
    let interval = Duration::from_millis(if index >= 4 {
        app.config.hardware_interval
    } else {
        app.config.interval
    });
    let window = interval.mul_f64(app.config.history as f64);
    let peak = history
        .points
        .iter()
        .filter_map(|point| point.values)
        .flat_map(|v| v.into_iter())
        .fold(0f64, f64::max);
    let (scale, unit, maximum) = match index {
        0 => (1., "CPU %", 100.),
        4 => (1., "GPU %", 100.),
        5 => (1., "C", (peak * 1.15).max(100.)),
        1 => {
            let total = app
                .live
                .as_ref()
                .and_then(|f| f.snapshot.memory.data.as_ref().ok())
                .map_or(1 << 30, |m| m.total_bytes);
            (1073741824., "RAM GiB", total as f64 / 1073741824.)
        }
        _ => {
            let (scale, unit) = if peak >= 1048576. {
                (1048576., "MiB/s")
            } else {
                (1024., "KiB/s")
            };
            (scale, unit, (peak / scale * 1.15).max(1.))
        }
    };
    let chart_now = area_chart::scroll_end(
        app.now,
        app.chart_origin,
        window,
        if app.ascii {
            area.width.min(120)
        } else {
            area.width
        },
    );
    let first = history.segments(chart_now, window, interval, 0, scale);
    let second = if matches!(index, 2 | 3) {
        history.segments(chart_now, window, interval, 1, scale)
    } else {
        Vec::new()
    };
    let name = if index == 2 {
        "R/W"
    } else if index == 3 {
        "RX/TX"
    } else {
        ""
    };
    let title = format!(
        "{:.0}s / {name} {unit} / max {maximum:.1}",
        window.as_secs_f64()
    );
    if app.ascii {
        // Fixed buckets by elapsed time, including blank buckets for missing samples.
        let width = area.width.min(120) as usize;
        let mut chars = vec![' '; width];
        for point in &history.points {
            if let Some(v) = point.values {
                let age = chart_now.saturating_duration_since(point.at).as_secs_f64();
                if age <= window.as_secs_f64() && width > 0 {
                    let x =
                        ((1. - age / window.as_secs_f64()) * (width - 1) as f64).round() as usize;
                    let ratio = v[0] / scale / maximum;
                    chars[x] = if ratio < 0.25 {
                        '.'
                    } else if ratio < 0.5 {
                        ':'
                    } else if ratio < 0.75 {
                        '*'
                    } else {
                        '#'
                    };
                }
            }
        }
        frame.render_widget(
            Paragraph::new(format!(
                "{title}\n{}",
                chars.into_iter().collect::<String>()
            ))
            .style(Style::default().fg(accent)),
            area,
        );
        return;
    }
    let regions = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).split(area);
    frame.render_widget(
        Paragraph::new(title).style(Style::default().fg(p.muted)),
        regions[0],
    );
    frame.render_widget(
        area_chart::AreaChart {
            first: &first,
            second: &second,
            x_bounds: [-window.as_secs_f64(), 0.],
            maximum,
            accent,
            palette: p,
        },
        regions[1],
    );
}

pub(super) fn hardware_panel(
    frame: &mut Frame,
    area: Rect,
    index: usize,
    app: &App,
    p: &Palette,
    detail: bool,
) {
    let accent = p.sections[index];
    let Some(h) = &app.hardware else {
        frame.render_widget(
            Paragraph::new("Collecting hardware... / rtop doctor")
                .style(Style::default().fg(p.muted)),
            area,
        );
        return;
    };
    let mut lines = Vec::new();
    let mut summary = if index == 1 {
        "N/D: no supported GPU".to_string()
    } else {
        "N/D: no sensors".to_string()
    };
    if index == 1 {
        if let Some(g) = app.gpu() {
            summary = g
                .utilization
                .as_ref()
                .map(|v| format!("{v:.1}% GPU"))
                .unwrap_or_else(|e| {
                    if matches!(e.as_str(), "collecting" | "reset" | "no progress") {
                        e.clone()
                    } else {
                        format!("N/D: {e}")
                    }
                });
            lines.push(format!("{} / {}", g.name, g.backend));
            lines.push(
                g.memory
                    .as_ref()
                    .map(|(u, t)| format!("VRAM {} / {}", bytes(*u), bytes(*t)))
                    .unwrap_or_else(|e| format!("VRAM N/D: {e}")),
            );
            lines.push(
                g.power_watts
                    .as_ref()
                    .map(|v| format!("Power {v:.1} W"))
                    .unwrap_or_else(|e| format!("Power N/D: {e}")),
            );
            lines.push(format!("ID {}", g.id));
            for s in h.sensors.iter().filter(|s| s.device == g.id) {
                lines.push(sensor_line(s));
            }
        }
        lines.push(format!(
            "{} GPU devices / [ ] select / rtop doctor",
            h.gpus.len()
        ));
    } else {
        if let Some(s) = app.sensor() {
            summary = s
                .celsius
                .as_ref()
                .map(|v| format!("{v:.1} C / {} {}", s.kind, s.label))
                .unwrap_or_else(|e| format!("N/D: {e}"));
            lines.push(format!("{} / {}", s.source, s.device));
        }
        for s in &h.sensors {
            lines.push(sensor_line(s));
        }
        lines.push(format!(
            "{} sensors / [ ] select / rtop doctor",
            h.sensors.len()
        ));
    }
    if h.gpus.is_empty() && index == 1 || h.sensors.is_empty() && index == 5 {
        lines.extend(h.diagnostics.iter().cloned());
    }
    if app.hardware_stale() {
        summary = format!("STALE {summary}");
    }
    let stats = if detail {
        lines
            .len()
            .min(area.height.saturating_sub(5) as usize)
            .max(1)
    } else {
        2
    };
    let areas = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(stats as u16),
        Constraint::Min(2),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(summary).style(Style::default().fg(accent).bold()),
        areas[0],
    );
    frame.render_widget(
        Paragraph::new(lines.into_iter().map(Line::from).collect::<Vec<_>>())
            .style(Style::default().fg(p.fg)),
        areas[1],
    );
    chart(
        frame,
        areas[2],
        if index == 1 { 4 } else { 5 },
        app,
        p,
        accent,
    );
}
fn sensor_line(s: &crate::hardware::Sensor) -> String {
    let value = s
        .celsius
        .as_ref()
        .map(|v| format!("{v:.1} C"))
        .unwrap_or_else(|e| format!("N/D: {e}"));
    format!(
        "{} {} / {}: {}{}",
        s.kind,
        s.label,
        s.source,
        value,
        s.critical
            .map_or(String::new(), |v| format!(" / crit {v:.1} C"))
    )
}
