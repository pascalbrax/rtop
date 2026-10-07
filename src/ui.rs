mod live;
mod process_table;
use crate::app::{App, SECTIONS};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{
        Axis, Block, Borders, Chart, Clear, Dataset, GraphType, Paragraph, Row, Table, Tabs, Wrap,
    },
};
const ASCII_BORDER: ratatui::symbols::border::Set = ratatui::symbols::border::Set {
    top_left: "+",
    top_right: "+",
    bottom_left: "+",
    bottom_right: "+",
    vertical_left: "|",
    vertical_right: "|",
    horizontal_top: "-",
    horizontal_bottom: "-",
};
fn border(app: &App) -> ratatui::symbols::border::Set {
    if app.ascii {
        ASCII_BORDER
    } else {
        ratatui::symbols::border::ROUNDED
    }
}

struct Palette {
    bg: Color,
    fg: Color,
    surface: Color,
    muted: Color,
    accent: Color,
    sections: [Color; 7],
}
impl Palette {
    fn new(app: &App) -> Self {
        if app.no_color {
            return Self {
                bg: Color::Reset,
                fg: Color::Reset,
                surface: Color::Reset,
                muted: Color::Reset,
                accent: Color::Reset,
                sections: [Color::Reset; 7],
            };
        }
        if app.light {
            Self {
                bg: Color::Rgb(245, 247, 250),
                fg: Color::Rgb(29, 40, 57),
                surface: Color::Rgb(255, 255, 255),
                muted: Color::Rgb(78, 94, 112),
                accent: Color::Rgb(55, 112, 36),
                sections: [
                    Color::Rgb(55, 112, 36),  // CPU: green
                    Color::Rgb(45, 91, 176),  // GPU: blue
                    Color::Rgb(130, 48, 164), // RAM: purple
                    Color::Rgb(0, 110, 100),  // Storage: teal
                    Color::Rgb(0, 102, 143),  // Network: cyan
                    Color::Rgb(152, 84, 15),  // Thermals: amber
                    Color::Rgb(117, 105, 0),  // Processes: yellow
                ],
            }
        } else {
            Self {
                bg: Color::Rgb(36, 37, 34),
                fg: Color::Rgb(226, 228, 222),
                surface: Color::Rgb(29, 30, 28),
                muted: Color::Rgb(151, 155, 147),
                accent: Color::Rgb(133, 205, 105),
                sections: [
                    Color::Rgb(133, 205, 105), // CPU: green
                    Color::Rgb(112, 156, 248), // GPU: blue
                    Color::Rgb(199, 112, 238), // RAM: purple
                    Color::Rgb(90, 197, 159),  // Storage: teal
                    Color::Rgb(102, 207, 231), // Network: cyan
                    Color::Rgb(232, 165, 82),  // Thermals: amber
                    Color::Rgb(215, 207, 105), // Processes: yellow
                ],
            }
        }
    }
}
pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let p = Palette::new(app);
    frame.render_widget(
        Block::default().style(Style::default().bg(p.bg).fg(p.fg)),
        area,
    );
    if area.width < 60 || area.height < 18 {
        frame.render_widget(
            Paragraph::new(format!(
                "rtop / {}\nTerminal too small.\nMinimum: 60 x 18\nResize or press q to exit.",
                if app.demo { "SIMULATED DATA" } else { "LIVE" }
            ))
            .wrap(Wrap { trim: false }),
            area,
        );
        return;
    }
    let rows = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Min(10),
        Constraint::Length(2),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(" rtop ", Style::default().fg(p.accent).bold()),
                Span::raw("/ observatory"),
                Span::styled(
                    if app.demo {
                        "     SIMULATED DATA"
                    } else {
                        "     LIVE / CPU RAM GPU DISK NET THERMALS"
                    },
                    Style::default().fg(p.muted),
                ),
            ]),
            Line::from(Span::styled(
                if app.demo {
                    format!(
                        " linux-demo | sample {:04} | {}",
                        app.sample,
                        if app.paused { "PAUSED" } else { "DEMO RUNNING" }
                    )
                } else {
                    format!(
                        " {} | {}",
                        app.hostname,
                        if app.paused {
                            "PAUSED"
                        } else if app.stale() {
                            "STALE"
                        } else if app.live.is_none() {
                            "COLLECTING"
                        } else {
                            "LIVE"
                        }
                    )
                },
                Style::default().fg(p.muted),
            )),
        ]),
        rows[0],
    );
    frame.render_widget(
        Tabs::new(SECTIONS.iter().enumerate().map(|(i, s)| {
            Line::from(Span::styled(
                format!("{} {}", i + 1, s),
                Style::default().fg(p.sections[i]),
            ))
        }))
        .select(app.selected)
        .divider(" ")
        .highlight_style(
            Style::default()
                .fg(p.sections[app.selected])
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ),
        rows[1],
    );
    let body = rows[2];
    if app.focused || area.width < 100 || area.height < 32 {
        panel(frame, body, app.selected, app, &p, true);
    } else {
        let bands = Layout::vertical([
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .split(body);
        let top = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .spacing(1)
            .split(bands[0]);
        panel(frame, top[0], 0, app, &p, false);
        panel(frame, top[1], 1, app, &p, false);
        let middle = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .spacing(1)
            .split(bands[1]);
        panel(frame, middle[0], 2, app, &p, false);
        panel(frame, middle[1], 6, app, &p, false);
        let bottom = Layout::horizontal([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .spacing(1)
        .split(bands[2]);
        for (i, area) in bottom.iter().enumerate() {
            panel(frame, *area, i + 3, app, &p, false);
        }
    }
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(" q quit  ? help  Tab select  Enter focus  Space pause"),
            Line::from(Span::styled(
                if app.demo {
                    " t theme   a ASCII   c color   |   SIMULATED DATA"
                } else {
                    " t theme  a ASCII  c color  [ ] device  f mount"
                },
                Style::default().fg(p.muted),
            )),
        ]),
        rows[3],
    );
    if app.help {
        let popup = Rect::new(
            area.x + (area.width - 58) / 2,
            area.y + (area.height - 16) / 2,
            58,
            16,
        );
        frame.render_widget(Clear, popup);
        frame.render_widget(Paragraph::new("KEYBOARD\n\nTab / arrows     Select section\n1 .. 7           Jump to section\nEnter / Esc      Focus / overview\nSpace            Pause sampling and redraws\nt                Light / dark theme\na                ASCII / Unicode\nc                Toggle colors\n? / Esc          Open / close help\nq / Ctrl-C       Quit\n\nProcesses: / filter, s sort, r reverse\nUp/Down, PgUp/PgDn (15), Home/End scroll")
            .style(Style::default().bg(p.bg).fg(p.fg)).block(Block::bordered().title(" Help ").border_set(border(app))), popup);
    }
}
fn panel(frame: &mut Frame, area: Rect, index: usize, app: &App, p: &Palette, detail: bool) {
    let selected = index == app.selected;
    let accent = p.sections[index];
    let block = Block::default()
        .style(Style::default().bg(p.surface).fg(p.fg))
        .borders(Borders::ALL)
        .border_set(border(app))
        .border_style(Style::default().fg(accent))
        .title(Line::from(Span::styled(
            format!(
                " {} {}{} ",
                index + 1,
                SECTIONS[index],
                if app.demo && index == 6 {
                    " / DEMO"
                } else {
                    ""
                }
            ),
            Style::default()
                .fg(accent)
                .bold()
                .add_modifier(if selected {
                    Modifier::UNDERLINED
                } else {
                    Modifier::empty()
                }),
        )));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if !app.demo && matches!(index, 1 | 5) {
        live::hardware_panel(frame, inner, index, app, p, detail);
        return;
    }
    if !app.demo && matches!(index, 0 | 2 | 3 | 4) {
        live::panel(frame, inner, index, app, p, detail);
        return;
    }
    if index == 6 {
        if app.demo {
            process_table(frame, inner, p);
        } else {
            process_table::draw(frame, inner, app, p);
        }
        return;
    }
    let usage = match index {
        0 => app.cpu(),
        1 => 67,
        2 => 43,
        3 => 61,
        4 => 28,
        _ => 52,
    };
    let lines: Vec<Line> = match index {
        0 => vec![
            format!("{:>3}%  total     3.42 GHz", app.cpu()).into(),
            "Load   1.42 / 1.18 / 0.96".into(),
            "Cores  24  38  62  17  45  28  31  19 %".into(),
            "8 logical cores / demo processor".into(),
        ],
        1 => vec![
            " 67%  GPU       4.2 / 8.0 GiB VRAM".into(),
            "Demo GPU        62 C      118 W".into(),
            "Core 1830 MHz   Fan 42%".into(),
            "Hotspot N/D     unavailable sensor".into(),
        ],
        2 => vec![
            " 43%  memory    13.8 / 32.0 GiB".into(),
            "Available       18.2 GiB".into(),
            "Cache            4.6 GiB".into(),
            "Swap             0.2 / 8.0 GiB".into(),
        ],
        3 => vec![
            " 61%  /         284 / 465 GiB".into(),
            "nvme0n1   R 12.4 MiB/s  W 3.1 MiB/s".into(),
            "/home     126 / 465 GiB".into(),
            "External volume with a very long name: N/D".into(),
        ],
        4 => vec![
            "eth0      RX 2.8 MiB/s".into(),
            "          TX 184 KiB/s".into(),
            "Total RX  8.4 GiB     TX 1.2 GiB".into(),
            "wlan0     disconnected".into(),
        ],
        _ => vec![
            "CPU package     52 C".into(),
            "CPU cores       44 - 51 C".into(),
            "GPU edge        62 C".into(),
            "GPU hotspot     N/D (not supported)".into(),
        ],
    };
    let stats_height = if detail { 4 } else { 2 };
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(stats_height),
        Constraint::Min(2),
    ])
    .split(inner);
    let summary = match index {
        4 => "RX 2.8 MiB/s / TX 184 KiB/s".to_string(),
        5 => "CPU 52 C / GPU 62 C".to_string(),
        _ => format!("{usage}%"),
    };
    let segment_width = chunks[0].width.saturating_sub(summary.len() as u16 + 2);
    let segments = (segment_width / 2) as usize;
    let filled = segments * usage as usize / 100;
    let dim = blend(accent, p.surface, 0.22);
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
                    ". ".repeat(segments - filled)
                } else {
                    glyph.repeat(segments - filled)
                },
                Style::default().fg(dim),
            ),
            Span::styled(format!(" {summary}"), Style::default().fg(accent).bold()),
        ])),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new(
            lines
                .into_iter()
                .take(stats_height as usize)
                .collect::<Vec<_>>(),
        )
        .style(Style::default().fg(p.fg)),
        chunks[1],
    );
    trace(frame, chunks[2], index, app, p);
}
// Already ordered by CPU: deterministic M1 fixture, no process scans or sorting on redraw.
const DEMO_PROCESSES: [(u32, &str, &str, &str); 8] = [
    (4217, "rustc", "42.3", "812 MiB"),
    (1832, "firefox", "18.7", "1.6 GiB"),
    (2974, "code", "8.4", "644 MiB"),
    (1128, "gnome-shell", "4.2", "286 MiB"),
    (3651, "pipewire", "1.6", "48 MiB"),
    (5072, "terminal", "0.8", "92 MiB"),
    (6304, "rtop-demo", "0.3", "12 MiB"),
    (902, "systemd", "0.1", "18 MiB"),
];
fn process_table(frame: &mut Frame, area: Rect, p: &Palette) {
    let accent = p.sections[6];
    let rows = DEMO_PROCESSES
        .iter()
        .enumerate()
        .map(|(i, (pid, name, cpu, memory))| {
            let style = if i == 0 {
                Style::default()
                    .fg(accent)
                    .bold()
                    .bg(blend(accent, p.surface, 0.15))
            } else if i % 2 == 0 {
                Style::default().bg(blend(accent, p.surface, 0.06))
            } else {
                Style::default()
            };
            Row::new(vec![
                pid.to_string(),
                name.to_string(),
                cpu.to_string(),
                memory.to_string(),
            ])
            .style(style)
        });
    let header = Row::new(["PID", "Name", "CPU%", "RAM"])
        .style(Style::default().fg(p.muted).bold())
        .bottom_margin(1);
    let title = "CPU descending / SIMULATED";
    let regions = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(area);
    frame.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(6),
                Constraint::Min(10),
                Constraint::Length(6),
                Constraint::Length(8),
            ],
        )
        .column_spacing(1)
        .header(header),
        regions[0],
    );
    frame.render_widget(
        Paragraph::new(title).style(Style::default().fg(p.muted)),
        regions[1],
    );
}
fn blend(foreground: Color, background: Color, amount: f64) -> Color {
    match (foreground, background) {
        (Color::Rgb(r, g, b), Color::Rgb(br, bg, bb)) => {
            let mix = |a: u8, b: u8| (f64::from(a) * amount + f64::from(b) * (1.0 - amount)) as u8;
            Color::Rgb(mix(r, br), mix(g, bg), mix(b, bb))
        }
        _ => Color::Reset,
    }
}
fn trace(frame: &mut Frame, area: Rect, index: usize, app: &App, p: &Palette) {
    let accent = p.sections[index];
    if app.ascii {
        let width = (area.width as usize).min(app.history.len());
        let graph: String = app.history[app.history.len() - width..]
            .iter()
            .map(|v| match v {
                0..=25 => '.',
                26..=40 => ':',
                41..=55 => '*',
                _ => '#',
            })
            .collect();
        frame.render_widget(
            Paragraph::new(format!(
                "Demo history / normalized
{graph}"
            ))
            .style(Style::default().fg(accent)),
            area,
        );
        return;
    }
    let values: Vec<(f64, f64)> = app
        .history
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let normalized = match index {
                1 => 40 + v / 2,
                2 => 36 + v / 5,
                3 => v / 2,
                4 => v / 3,
                5 => 45 + v / 8,
                _ => *v,
            };
            (i as f64, normalized as f64)
        })
        .collect();
    let grid = blend(accent, p.surface, 0.17);
    let horizontal: Vec<Vec<(f64, f64)>> = [25.0, 50.0, 75.0]
        .into_iter()
        .map(|y| vec![(0.0, y), (59.0, y)])
        .collect();
    let vertical: Vec<Vec<(f64, f64)>> = [10.0, 20.0, 30.0, 40.0, 50.0]
        .into_iter()
        .map(|x| vec![(x, 0.0), (x, 100.0)])
        .collect();
    let mut datasets: Vec<Dataset> = horizontal
        .iter()
        .chain(vertical.iter())
        .map(|points| {
            Dataset::default()
                .graph_type(GraphType::Line)
                .marker(ratatui::symbols::Marker::Braille)
                .style(Style::default().fg(grid))
                .data(points)
        })
        .collect();
    datasets.push(
        Dataset::default()
            .graph_type(GraphType::Line)
            .marker(ratatui::symbols::Marker::Braille)
            .style(Style::default().fg(accent))
            .data(&values),
    );
    frame.render_widget(
        Chart::new(datasets)
            .block(Block::default().title(Span::styled(
                "Demo history / normalized",
                Style::default().fg(p.muted),
            )))
            .x_axis(
                Axis::default()
                    .bounds([0.0, 59.0])
                    .style(Style::default().fg(grid)),
            )
            .y_axis(
                Axis::default()
                    .bounds([0.0, 100.0])
                    .style(Style::default().fg(grid)),
            )
            .style(Style::default().bg(p.surface)),
        area,
    );
}
pub fn buffer_text(buffer: &Buffer) -> String {
    let mut output = String::new();
    for y in buffer.area.y..buffer.area.bottom() {
        for x in buffer.area.x..buffer.area.right() {
            output.push_str(buffer[(x, y)].symbol());
        }
        output.push('\n');
    }
    output
}
/// Capture the rendered cells, including palette, without a display server.
pub fn buffer_svg(buffer: &Buffer) -> String {
    use std::fmt::Write;
    fn color(c: Color, fallback: &str) -> String {
        match c {
            Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
            _ => fallback.into(),
        }
    }
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">\n",
        buffer.area.width as u32 * 9,
        buffer.area.height as u32 * 18,
        buffer.area.width as u32 * 9,
        buffer.area.height as u32 * 18
    );
    svg.push_str("<title>rtop — terminal capture</title>\n");
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let cell = &buffer[(x, y)];
            let bg = color(cell.bg, "#0f141d");
            let fg = color(cell.fg, "#dde7f1");
            let symbol = cell
                .symbol()
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            let _ = writeln!(
                svg,
                "<rect x=\"{}\" y=\"{}\" width=\"9\" height=\"18\" fill=\"{bg}\"/>",
                x as u32 * 9,
                y as u32 * 18
            );
            if symbol != " " {
                let weight = if cell.modifier.contains(Modifier::BOLD) {
                    "bold"
                } else {
                    "normal"
                };
                let _ = writeln!(
                    svg,
                    "<text x=\"{}\" y=\"{}\" fill=\"{fg}\" font-family=\"DejaVu Sans Mono,monospace\" font-size=\"14\" font-weight=\"{weight}\">{symbol}</text>",
                    x as u32 * 9,
                    y as u32 * 18 + 14
                );
            }
        }
    }
    svg.push_str("</svg>");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};
    #[test]
    fn palette_text_contrast_and_monochrome() {
        fn luminance(c: Color) -> f64 {
            let Color::Rgb(r, g, b) = c else {
                panic!("RGB required")
            };
            let linear = |v: u8| {
                let v = f64::from(v) / 255.;
                if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
        }
        let contrast = |a, b| {
            let (a, b) = (luminance(a), luminance(b));
            (a.max(b) + 0.05) / (a.min(b) + 0.05)
        };
        for light in [false, true] {
            let app = App::new(light, false, false);
            let p = Palette::new(&app);
            for fg in [p.fg, p.muted].into_iter().chain(p.sections) {
                for bg in [p.bg, p.surface] {
                    assert!(
                        contrast(fg, bg) >= 4.5,
                        "insufficient text contrast: {fg:?}/{bg:?}"
                    );
                }
            }
            for (i, color) in p.sections.iter().enumerate() {
                assert!(!p.sections[..i].contains(color));
            }
        }
        let app = App::new(false, false, true);
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        terminal.draw(|f| draw(f, &app)).unwrap();
        for cell in &terminal.backend().buffer().content {
            assert_eq!(cell.fg, Color::Reset);
            assert_eq!(cell.bg, Color::Reset);
        }
    }
    #[test]
    fn real_process_ascii_errors_and_rows() {
        let mut app = App::configured(
            crate::config::Config {
                ascii: true,
                ..Default::default()
            },
            false,
        );
        app.selected = 6;
        app.processes.apply(crate::processes::Frame {
            skipped: 2,
            observation: crate::model::Observation {
                at: std::time::Instant::now(),
                cost: std::time::Duration::ZERO,
                data: Ok(vec![crate::processes::Process {
                    id: crate::processes::Identity { pid: 123, start: 1 },
                    name: "name-�".into(),
                    cpu: crate::model::Rate::Value(250.),
                    rss: 1 << 20,
                }]),
            },
        });
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| draw(f, &app)).unwrap();
        let text = buffer_text(terminal.backend().buffer());
        assert!(text.is_ascii());
        assert!(text.contains("name-?"));
        assert!(text.contains("250.0"));
        assert!(!text.contains("DEMO"));
        app.processes.frame.as_mut().unwrap().observation.data = Err("permission denied".into());
        terminal.draw(|f| draw(f, &app)).unwrap();
        assert!(buffer_text(terminal.backend().buffer()).contains("N/D: permission denied"));
    }
    #[test]
    fn layouts_themes_and_sections() {
        for (w, h) in [(80, 24), (120, 40), (160, 50), (60, 18)] {
            for (light, ascii, no_color) in [
                (false, false, false),
                (true, false, false),
                (false, true, true),
                (false, false, true),
            ] {
                for (section, name) in SECTIONS.iter().enumerate() {
                    let mut app = App::new(light, ascii, no_color);
                    app.selected = section;
                    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
                    terminal.draw(|f| draw(f, &app)).unwrap();
                    let text = buffer_text(terminal.backend().buffer());
                    assert!(text.contains("SIMULATED DATA"));
                    assert!(text.contains(&format!("{} {}", section + 1, name)));
                    assert!(text.contains("q quit"));
                    if w >= 100 {
                        for s in SECTIONS {
                            assert!(text.contains(s));
                        }
                    }
                    if ascii {
                        assert!(text.is_ascii());
                    }
                }
            }
        }
    }
    #[test]
    fn live_errors_staleness_and_source_labels() {
        use std::time::{Duration, Instant};
        let mut app = App::configured(crate::config::Config::default(), false);
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        app.apply(crate::model::fixture(Instant::now()));
        terminal.draw(|f| draw(f, &app)).unwrap();
        let text = buffer_text(terminal.backend().buffer());
        for label in [
            "LIVE / CPU RAM GPU DISK NET THERMALS",
            "Processes",
            "Collecting processes",
            "eth0",
            "sda",
            "2.0 GiB",
            "40.0%",
            "120s",
        ] {
            assert!(text.contains(label), "missing {label}");
        }
        let mut frame = crate::model::fixture(Instant::now());
        frame.snapshot.memory.data = Err("permission denied".into());
        app.apply(frame);
        terminal.draw(|f| draw(f, &app)).unwrap();
        let text = buffer_text(terminal.backend().buffer());
        assert!(text.contains("Memory N/D: permission denied"));
        assert!(text.contains("eth0"));
        app.now = app.live.as_ref().unwrap().snapshot.at + Duration::from_secs(4);
        terminal.draw(|f| draw(f, &app)).unwrap();
        assert!(buffer_text(terminal.backend().buffer()).contains("STALE"));
        app.paused = true;
        assert!(!app.stale());
        app.selected = 4;
        app.focused = true;
        app.interface = Some("removed".into());
        terminal.draw(|f| draw(f, &app)).unwrap();
        assert!(
            buffer_text(terminal.backend().buffer()).contains("Network N/D: interface unavailable")
        );
    }
    #[test]
    fn hardware_absence_errors_selection_and_staleness() {
        use crate::hardware::{Gpu, HardwareFrame, Sensor};
        use std::time::{Duration, Instant};
        for (w, h) in [(80, 24), (120, 40), (160, 50)] {
            for (light, ascii, no_color) in [
                (false, false, false),
                (true, false, false),
                (false, true, true),
            ] {
                let mut app = App::configured(crate::config::Config::default(), false);
                app.light = light;
                app.ascii = ascii;
                app.no_color = no_color;
                app.apply(crate::model::fixture(Instant::now()));
                app.apply_hardware(HardwareFrame {
                    at: Instant::now(),
                    gpu_cost: Duration::ZERO,
                    sensor_cost: Duration::ZERO,
                    gpus: vec![],
                    sensors: vec![],
                    diagnostics: vec!["NVML library unavailable".into()],
                });
                for selected in [1, 5] {
                    app.selected = selected;
                    app.focused = true;
                    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
                    terminal.draw(|f| draw(f, &app)).unwrap();
                    let text = buffer_text(terminal.backend().buffer());
                    assert!(text.contains("N/D:"));
                    assert!(!text.contains("/ DEMO"));
                    if ascii {
                        assert!(text.is_ascii());
                    }
                }
            }
        }
        let mut app = App::configured(crate::config::Config::default(), false);
        app.selected = 1;
        app.focused = true;
        app.apply_hardware(HardwareFrame {
            at: Instant::now(),
            gpu_cost: Duration::ZERO,
            sensor_cost: Duration::ZERO,
            gpus: vec![Gpu {
                id: "gpu0".into(),
                name: "GPU test".into(),
                backend: "NVML",
                utilization: Err("GPU Lost".into()),
                memory: Err("Not Supported".into()),
                power_watts: Err("Not Supported".into()),
            }],
            sensors: vec![
                Sensor {
                    id: "s0".into(),
                    device: "gpu0".into(),
                    label: "GPU core".into(),
                    kind: "GPU",
                    source: "NVML".into(),
                    celsius: Ok(61.),
                    critical: None,
                },
                Sensor {
                    id: "s1".into(),
                    device: "cpu0".into(),
                    label: "Package id 0".into(),
                    kind: "CPU",
                    source: "coretemp".into(),
                    celsius: Err("permission denied".into()),
                    critical: Some(100.),
                },
            ],
            diagnostics: vec![],
        });
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        terminal.draw(|f| draw(f, &app)).unwrap();
        assert!(buffer_text(terminal.backend().buffer()).contains("N/D: GPU Lost"));
        app.selected = 5;
        assert_eq!(app.sensor().unwrap().id, "s1");
        app.key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char(']'),
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(app.sensor().unwrap().id, "s0");
        app.now = app.hardware.as_ref().unwrap().at + Duration::from_secs(7);
        terminal.draw(|f| draw(f, &app)).unwrap();
        assert!(buffer_text(terminal.backend().buffer()).contains("STALE"));
    }
    #[test]
    fn small_terminal_and_help() {
        let mut terminal = Terminal::new(TestBackend::new(40, 10)).unwrap();
        let mut app = App::new(false, false, false);
        terminal.draw(|f| draw(f, &app)).unwrap();
        assert!(buffer_text(terminal.backend().buffer()).contains("Terminal too small"));
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        app.help = true;
        terminal.draw(|f| draw(f, &app)).unwrap();
        assert!(buffer_text(terminal.backend().buffer()).contains("KEYBOARD"));
    }
}
