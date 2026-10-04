use super::{Palette, blend};
use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};
use std::time::{Duration, Instant};

/// Move the entire viewport by whole columns, rather than rounding each sample's
/// fractional movement independently. Rounding up keeps the newest sample visible.
pub(super) fn scroll_end(now: Instant, origin: Instant, window: Duration, width: u16) -> Instant {
    if width <= 1 || window.is_zero() {
        return now;
    }
    let step = (window.as_nanos() / u128::from(width - 1)).max(1);
    let elapsed = now.saturating_duration_since(origin).as_nanos();
    let snapped = elapsed.div_ceil(step) * step;
    u64::try_from(snapped)
        .ok()
        .and_then(|nanos| origin.checked_add(Duration::from_nanos(nanos)))
        .unwrap_or(now)
}

/// A two-pixels-per-cell area chart. Work is bounded by the visible terminal area.
pub(super) struct AreaChart<'a> {
    pub first: &'a [Vec<(f64, f64)>],
    pub second: &'a [Vec<(f64, f64)>],
    pub x_bounds: [f64; 2],
    pub maximum: f64,
    pub accent: ratatui::style::Color,
    pub palette: &'a Palette,
}

#[derive(Clone, Copy)]
struct Column {
    height: usize,
    line_start: usize,
    line_end: usize,
}

fn columns(
    segments: &[Vec<(f64, f64)>],
    width: usize,
    pixels: usize,
    bounds: [f64; 2],
    maximum: f64,
) -> Vec<Option<Column>> {
    let mut output: Vec<Option<Column>> = vec![None; width];
    if width == 0 || pixels == 0 || bounds[1] <= bounds[0] || maximum <= 0. {
        return output;
    }
    let position = |(x, y): (f64, f64)| {
        if !x.is_finite() || !y.is_finite() || x < bounds[0] || x > bounds[1] {
            return None;
        }
        Some((
            ((x - bounds[0]) / (bounds[1] - bounds[0]) * (width - 1) as f64).round() as usize,
            ((1. - (y / maximum).clamp(0., 1.)) * (pixels - 1) as f64).round() as usize,
        ))
    };
    for segment in segments {
        let mut previous = None;
        for &point in segment {
            let Some((x, height)) = position(point) else {
                previous = None;
                continue;
            };
            let (start_x, start_height) = previous.unwrap_or((x, height));
            if start_x > x {
                previous = None;
                continue;
            }
            let mut last_height = start_height;
            for (col, slot) in output.iter_mut().enumerate().take(x + 1).skip(start_x) {
                let fraction = if x == start_x {
                    1.
                } else {
                    (col - start_x) as f64 / (x - start_x) as f64
                };
                let at = (start_height as f64 + (height as f64 - start_height as f64) * fraction)
                    .round() as usize;
                let next = Column {
                    height: at,
                    line_start: at.min(last_height),
                    line_end: at.max(last_height),
                };
                // Multiple samples in one cell retain the highest observed peak.
                *slot = Some(match *slot {
                    Some(old) => Column {
                        height: old.height.min(next.height),
                        line_start: old.line_start.min(next.line_start),
                        line_end: old.line_end.max(next.line_end),
                    },
                    None => next,
                });
                last_height = at;
            }
            previous = Some((x, height));
        }
    }
    output
}

impl Widget for AreaChart<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        let width = usize::from(area.width);
        let pixels = usize::from(area.height) * 2;
        let first = columns(self.first, width, pixels, self.x_bounds, self.maximum);
        let second = columns(self.second, width, pixels, self.x_bounds, self.maximum);
        let monochrome = self.accent == ratatui::style::Color::Reset;
        let colors = [
            self.palette.surface,
            blend(self.palette.fg, self.palette.surface, 0.12),
            blend(self.accent, self.palette.surface, 0.24),
            self.accent,
            blend(self.accent, self.palette.fg, 0.55),
        ];
        let pixel = |x: usize, y: usize| {
            let mut ink = if !monochrome && y == pixels / 2 { 1 } else { 0 };
            if let Some(col) = first[x] {
                if !monochrome && y >= col.height {
                    ink = 2;
                }
                if (col.line_start..=col.line_end).contains(&y) {
                    ink = 3;
                }
            }
            if second[x].is_some_and(|col| (col.line_start..=col.line_end).contains(&y)) {
                ink = 4;
            }
            ink
        };
        for y in 0..area.height {
            for x in 0..area.width {
                let top = pixel(usize::from(x), usize::from(y) * 2);
                let bottom = pixel(usize::from(x), usize::from(y) * 2 + 1);
                let (symbol, fg, bg) = if top == bottom {
                    if top == 0 {
                        (" ", colors[0], colors[0])
                    } else {
                        ("█", colors[top], colors[0])
                    }
                } else if top == 0 {
                    ("▄", colors[bottom], colors[0])
                } else {
                    ("▀", colors[top], colors[bottom])
                };
                let cell = &mut buf[(area.x + x, area.y + y)];
                cell.set_symbol(symbol)
                    .set_style(Style::default().fg(fg).bg(bg));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_series_scroll_as_one_image_without_deforming() {
        let origin = Instant::now();
        let window = Duration::from_secs(12);
        let palette = Palette::new(&crate::app::App::new(false, false, false));
        let draw = |seconds: f64, align: bool| {
            let end = if align {
                scroll_end(origin + Duration::from_secs_f64(seconds), origin, window, 9)
                    .duration_since(origin)
                    .as_secs_f64()
            } else {
                seconds
            };
            let first = vec![
                vec![(9., 90.), (10.1, 10.), (12.2, 75.), (14., 20.)]
                    .into_iter()
                    .map(|(at, value)| (at - end, value))
                    .collect(),
            ];
            let second = vec![
                vec![(9.3, 20.), (11.7, 80.), (13.1, 5.), (14.5, 40.)]
                    .into_iter()
                    .map(|(at, value)| (at - end, value))
                    .collect(),
            ];
            let area = Rect::new(0, 0, 9, 8);
            let mut buffer = Buffer::empty(area);
            AreaChart {
                first: &first,
                second: &second,
                x_bounds: [-12., 0.],
                maximum: 100.,
                accent: palette.sections[4],
                palette: &palette,
            }
            .render(area, &mut buffer);
            buffer
        };
        assert_ne!(
            draw(15.1, false),
            draw(15.4, false),
            "reproduce the old per-point rounding defect"
        );
        let before = draw(15.1, true);
        assert_eq!(
            before,
            draw(15.4, true),
            "sub-column elapsed time must not deform the image"
        );
        let after = draw(16.6, true);
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(
                    before[(x + 1, y)],
                    after[(x, y)],
                    "all rows and both series must shift together"
                );
            }
        }
        assert_eq!(scroll_end(origin, origin, window, 0), origin);
        assert_eq!(scroll_end(origin, origin, window, 1), origin);
    }

    #[test]
    fn gaps_single_samples_and_peaks_remain_visible() {
        let segments = vec![vec![(0., 25.), (1., 75.)], vec![(4., 50.)]];
        let cols = columns(&segments, 6, 8, [0., 5.], 100.);
        assert!(cols[0].is_some() && cols[1].is_some() && cols[4].is_some());
        assert!(cols[2].is_none() && cols[3].is_none() && cols[5].is_none());
        assert_eq!(cols[4].unwrap().line_start, cols[4].unwrap().line_end);
        let compressed = columns(&[vec![(0., 90.), (0.1, 10.)]], 2, 11, [0., 10.], 100.);
        assert_eq!(compressed[0].unwrap().height, 1);
    }
    #[test]
    fn rendering_preserves_blank_gaps_and_the_second_series() {
        let app = crate::app::App::new(false, false, false);
        let palette = Palette::new(&app);
        let accent = palette.sections[0];
        let area = Rect::new(10, 20, 6, 4);
        let mut buffer = Buffer::empty(area);
        AreaChart {
            first: &[vec![(0., 50.)], vec![(4., 50.)]],
            second: &[vec![(3., 25.)]],
            x_bounds: [0., 5.],
            maximum: 100.,
            accent,
            palette: &palette,
        }
        .render(area, &mut buffer);
        assert_eq!(buffer[(10, 23)].fg, blend(accent, palette.surface, 0.24));
        assert_eq!(buffer[(12, 23)].symbol(), " ");
        // The upper half is the middle grid reference; the lower half is series two.
        assert_eq!(buffer[(13, 22)].bg, blend(accent, palette.fg, 0.55));
        assert_eq!(buffer[(13, 22)].symbol(), "▀");

        let mono = Palette::new(&crate::app::App::new(false, false, true));
        AreaChart {
            first: &[vec![(0., 50.)]],
            second: &[],
            x_bounds: [0., 5.],
            maximum: 100.,
            accent: mono.sections[0],
            palette: &mono,
        }
        .render(area, &mut buffer);
        assert_eq!(buffer[(10, 23)].symbol(), " ");
        assert_eq!(buffer[(10, 22)].symbol(), "▀");
    }

    #[test]
    fn invalid_and_tiny_plots_do_not_panic() {
        let samples = [vec![(0., f64::NAN), (1., 300.), (2., -10.)]];
        assert!(columns(&samples, 0, 0, [0., 2.], 100.).is_empty());
        let cols = columns(&samples, 1, 2, [0., 2.], 100.);
        assert_eq!(cols[0].unwrap().height, 0);
        assert!(
            columns(&samples, 2, 2, [1., 1.], 100.)
                .iter()
                .all(Option::is_none)
        );
    }
}
