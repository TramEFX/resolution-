//! Video scopes for the viewers (LOG D-030) on FilmCraft's scope maths (`filmcraft-scopes`): the
//! frame is sampled (at most 480 × 270, nearest sample), counted into grids, and the grids are
//! painted with FilmCraft's phosphor-like intensity curve. The histogram is painted here, levels
//! across as in Resolve.

use filmcraft_scopes::{self as sc, Grid, ParadeType, Params, Signal, WaveformType};
use resolution_engine_api::{Frame, PARADE_GAP, ScopeKind, Scopes};

/// R, G and B traces.
const RED: [f32; 3] = [1.0, 0.33, 0.33];
const GREEN: [f32; 3] = [0.35, 1.0, 0.45];
const BLUE: [f32; 3] = [0.35, 0.55, 1.0];

fn colour(trace: &str) -> [f32; 3] {
    match trace {
        "R" => RED,
        "G" => GREEN,
        "B" => BLUE,
        _ => [1.0; 3],
    }
}

/// Histogram height, in rows.
const HISTOGRAM_ROWS: usize = 128;

pub struct FilmCraftScopes;

impl Scopes for FilmCraftScopes {
    fn draw(&self, frame: &Frame, kind: ScopeKind) -> Frame {
        let (w, h) = (frame.width as usize, frame.height as usize);
        if w == 0 || h == 0 || frame.rgba.len() < w * h * 4 {
            return Frame { width: 0, height: 0, rgba: Vec::new() };
        }
        let s = Signal::from_rgba8(w, h, &frame.rgba);
        let p = Params::default();
        let (width, height, rgba) = match kind {
            ScopeKind::Waveform => {
                let wf = sc::waveform(&s, WaveformType::Rgb, &p);
                let layers: Vec<(&Grid, [f32; 3])> = wf.traces.iter().map(|g| (g, colour(g.name))).collect();
                sc::paint::grids(&layers, sc::paint::waveform_reference(wf.per_column), 1.0)
            }
            ScopeKind::Parade => {
                let wf = sc::parade(&s, ParadeType::Rgb, &p);
                let reference = sc::paint::waveform_reference(wf.per_column);
                let panes: Vec<(usize, usize, Vec<u8>)> =
                    wf.traces.iter().map(|g| sc::paint::grids(&[(g, colour(g.name))], reference, 1.0)).collect();
                side_by_side(&panes, PARADE_GAP as usize)
            }
            ScopeKind::Vectorscope => sc::paint::vectorscope(&sc::vectorscope_yuv(&s, &p), [1.0; 3], 1.0, true),
            ScopeKind::Histogram => histogram(&sc::histogram(&s, &p)),
        };
        Frame { width: width as u32, height: height as u32, rgba }
    }
}

/// Pictures of one height side by side, `gap` transparent columns apart.
fn side_by_side(panes: &[(usize, usize, Vec<u8>)], gap: usize) -> (usize, usize, Vec<u8>) {
    let h = panes.iter().map(|p| p.1).max().unwrap_or(0);
    let w = panes.iter().map(|p| p.0).sum::<usize>() + gap * panes.len().saturating_sub(1);
    let mut out = vec![0u8; w * h * 4];
    let mut x0 = 0;
    for (pw, ph, px) in panes {
        for y in 0..(*ph).min(h) {
            let src = &px[y * pw * 4..(y + 1) * pw * 4];
            out[(y * w + x0) * 4..(y * w + x0 + pw) * 4].copy_from_slice(src);
        }
        x0 += pw + gap;
    }
    (w, h, out)
}

/// A histogram as Resolve draws it: one column per level (black on the left), R, G and B bars
/// rising from the bottom on a square-root scale and adding up where they overlap.
fn histogram(hist: &sc::Histogram) -> (usize, usize, Vec<u8>) {
    let (w, h) = (sc::BINS, HISTOGRAM_ROWS);
    let max = hist.r.iter().chain(&hist.g).chain(&hist.b).copied().max().unwrap_or(0).max(1) as f32;
    let height = |c: u32| ((c as f32 / max).sqrt() * h as f32).round() as usize;
    let mut out = vec![0u8; w * h * 4];
    for x in 0..w {
        let bars = [(height(hist.r[x]), RED), (height(hist.g[x]), GREEN), (height(hist.b[x]), BLUE)];
        for row in 0..h {
            let mut c = [0f32; 3];
            for (top, col) in bars {
                if row < top {
                    for k in 0..3 {
                        c[k] += col[k] * 0.7;
                    }
                }
            }
            let a = c[0].max(c[1]).max(c[2]).min(1.0);
            if a > 0.0 {
                let y = h - 1 - row;
                let i = (y * w + x) * 4;
                out[i..i + 4].copy_from_slice(&[
                    (c[0].min(1.0) * 255.0) as u8,
                    (c[1].min(1.0) * 255.0) as u8,
                    (c[2].min(1.0) * 255.0) as u8,
                    (a * 255.0) as u8,
                ]);
            }
        }
    }
    (w, h, out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// A flat-colour frame.
    fn flat(rgb: [u8; 3]) -> Frame {
        let (w, h) = (64u32, 36u32);
        Frame { width: w, height: h, rgba: (0..w * h).flat_map(|_| [rgb[0], rgb[1], rgb[2], 255]).collect() }
    }

    /// Rows (from the bottom) and columns that hold anything.
    fn lit(f: &Frame) -> Vec<(usize, usize)> {
        let w = f.width as usize;
        let h = f.height as usize;
        (0..h)
            .flat_map(|y| (0..w).map(move |x| (y, x)))
            .filter(|(y, x)| f.rgba[(y * w + x) * 4 + 3] > 0)
            .map(|(y, x)| (h - 1 - y, x))
            .collect()
    }

    #[test]
    fn a_grey_frame_draws_one_white_line_at_its_level() {
        let f = FilmCraftScopes.draw(&flat([128, 128, 128]), ScopeKind::Waveform);
        assert_eq!(f.height, 256, "a row per 8-bit level");
        let rows: std::collections::BTreeSet<usize> = lit(&f).into_iter().map(|(r, _)| r).collect();
        assert_eq!(rows.into_iter().collect::<Vec<_>>(), vec![128], "every column at level 128");
        // R, G and B agree: the line adds up to white
        let i = ((255 - 128) * f.width as usize) * 4;
        let px = &f.rgba[i..i + 3];
        assert!(px.iter().all(|v| *v > 200), "white where the traces agree: {px:?}");
    }

    #[test]
    fn the_parade_draws_three_panes_at_each_channels_level() {
        let f = FilmCraftScopes.draw(&flat([200, 100, 50]), ScopeKind::Parade);
        let pane = (f.width as usize - 2 * PARADE_GAP as usize) / 3;
        for (k, level) in [200usize, 100, 50].into_iter().enumerate() {
            let x0 = k * (pane + PARADE_GAP as usize);
            let rows: std::collections::BTreeSet<usize> =
                lit(&f).into_iter().filter(|(_, x)| (x0..x0 + pane).contains(x)).map(|(r, _)| r).collect();
            assert_eq!(rows.into_iter().collect::<Vec<_>>(), vec![level], "pane {k}");
        }
    }

    #[test]
    fn a_grey_frame_sits_in_the_middle_of_the_vectorscope_and_red_up_left() {
        let grey = FilmCraftScopes.draw(&flat([128, 128, 128]), ScopeKind::Vectorscope);
        let n = grey.width as usize;
        assert_eq!(n, grey.height as usize, "square");
        let centre = n / 2;
        assert!(
            lit(&grey).iter().all(|(r, x)| r.abs_diff(centre) <= 1 && x.abs_diff(centre) <= 1),
            "grey is at the centre"
        );
        // red: Cb below zero (left), Cr above (up)
        let red = FilmCraftScopes.draw(&flat([200, 30, 30]), ScopeKind::Vectorscope);
        let spot = lit(&red);
        let (r, x) = spot[spot.len() / 2];
        assert!(r > centre + 5 && x + 5 < centre, "red is up and to the left: row {r}, column {x}");
    }

    #[test]
    fn the_histogram_puts_each_channel_at_its_level() {
        let f = FilmCraftScopes.draw(&flat([200, 100, 50]), ScopeKind::Histogram);
        assert_eq!(f.width, 256);
        let columns: std::collections::BTreeSet<usize> = lit(&f).into_iter().map(|(_, x)| x).collect();
        assert_eq!(columns.into_iter().collect::<Vec<_>>(), vec![50, 100, 200]);
    }

    /// Agents read the scopes as numbers (`scopes.read`, FilmCraft's command) without changing anything.
    #[test]
    fn agents_read_the_scopes_as_numbers() {
        use resolution_engine_api::Engine;
        let mut e = crate::FilmCraftEngine::new();
        e.execute("raw.filmcraft/file.openDemoProject", serde_json::json!({})).unwrap();
        e.execute("playhead.set", serde_json::json!({"seconds": 2.0})).unwrap();
        let before = e.revision();
        let out = e.execute("scopes.read", serde_json::json!({})).unwrap();
        assert!(out.result.as_object().is_some_and(|o| !o.is_empty()), "{}", out.result);
        assert_eq!(out.undo_label, None);
        assert_eq!(e.revision(), before, "reading changes nothing");
    }

    #[test]
    fn an_empty_frame_draws_nothing() {
        let f = FilmCraftScopes.draw(&Frame { width: 0, height: 0, rgba: Vec::new() }, ScopeKind::Waveform);
        assert_eq!((f.width, f.height), (0, 0));
    }
}
