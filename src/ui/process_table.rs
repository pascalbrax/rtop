use super::*;
pub(super) fn draw(frame: &mut Frame, area: Rect, app: &App, p: &Palette) {
    let view = &app.processes;
    let regions = Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).split(area);
    let mut info = if let Some(f) = &view.frame {
        match &f.observation.data {
            Ok(v) => format!(
                "{} / {} · {} {} · skipped {}",
                view.rows.len(),
                v.len(),
                view.sort.label(),
                if view.descending { "desc" } else { "asc" },
                f.skipped
            ),
            Err(e) => format!("N/D: {e}"),
        }
    } else {
        "Collecting processes...".into()
    };
    if !app.paused
        && view.frame.as_ref().is_some_and(|f| {
            app.now
                .saturating_duration_since(f.observation.at)
                .as_millis()
                > u128::from(app.config.process_interval) * 3
        })
    {
        info = format!("STALE / {info}");
    }
    if let Some(processes) = view
        .frame
        .as_ref()
        .and_then(|f| f.observation.data.as_ref().ok())
    {
        let count = usize::from(regions[0].height.saturating_sub(1));
        let start = if app.selected != 6 {
            0
        } else {
            view.cursor
                .saturating_sub(count / 2)
                .min(view.rows.len().saturating_sub(count))
        };
        let rows = view
            .rows
            .iter()
            .enumerate()
            .skip(start)
            .take(count)
            .map(|(position, index)| {
                let process = &processes[*index];
                let cpu = match process.cpu {
                    crate::model::Rate::Value(v) => format!("{v:.1}"),
                    crate::model::Rate::Sampling => "...".into(),
                    crate::model::Rate::Reset => "reset".into(),
                    crate::model::Rate::NoProgress => "N/D".into(),
                };
                let rss = format!("{:.1} MiB", process.rss as f64 / 1048576.);
                let style = if position == view.cursor {
                    Style::default().fg(p.sections[6]).bold().bg(blend(
                        p.sections[6],
                        p.surface,
                        0.15,
                    ))
                } else if position % 2 == 0 {
                    Style::default().bg(blend(p.sections[6], p.surface, 0.06))
                } else {
                    Style::default()
                };
                let name = if app.ascii {
                    process
                        .name
                        .chars()
                        .map(|c| if c.is_ascii() { c } else { '?' })
                        .collect()
                } else {
                    process.name.clone()
                };
                Row::new([process.id.pid.to_string(), name, cpu, rss]).style(style)
            });
        frame.render_widget(
            Table::new(
                rows,
                [
                    Constraint::Length(7),
                    Constraint::Min(8),
                    Constraint::Length(7),
                    Constraint::Length(11),
                ],
            )
            .header(
                Row::new(["PID", "Name", "CPU%", "RSS"]).style(Style::default().fg(p.muted).bold()),
            )
            .column_spacing(1),
            regions[0],
        );
    }
    let status = if view.editing {
        format!("Filter: {}_  Enter apply / Esc clear", view.filter)
    } else if !view.filter.is_empty() {
        format!("Filter: {}  / edit · s sort · r reverse", view.filter)
    } else {
        "/ filter · s sort · r reverse · arrows scroll".into()
    };
    let ascii = |text: String| {
        if app.ascii {
            text.chars()
                .map(|c| if c.is_ascii() { c } else { ' ' })
                .collect()
        } else {
            text
        }
    };
    frame.render_widget(
        Paragraph::new(vec![Line::from(ascii(info)), Line::from(ascii(status))])
            .style(Style::default().fg(p.muted)),
        regions[1],
    );
}
