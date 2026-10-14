//! Video scopes for the viewers (LOG D-030): pictures of a frame's levels and colours, drawn by
//! the engine, so a front end only places them and draws the graticule over them.

use crate::Frame;

/// The scopes the Color page docks, as Resolve names them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ScopeKind {
    /// R, G and B levels across the picture, overlaid and tinted with their colours: white where
    /// they agree.
    #[default]
    Waveform,
    /// R, G and B levels side by side.
    Parade,
    /// Hue as the angle and saturation as the distance from the centre: Rec. 709 Cb across, Cr up.
    Vectorscope,
    /// How many pixels of R, G and B sit at each level, black on the left.
    Histogram,
}

impl ScopeKind {
    pub const ALL: [ScopeKind; 4] =
        [ScopeKind::Waveform, ScopeKind::Parade, ScopeKind::Vectorscope, ScopeKind::Histogram];

    pub fn label(self) -> &'static str {
        match self {
            ScopeKind::Waveform => "Waveform",
            ScopeKind::Parade => "Parade",
            ScopeKind::Vectorscope => "Vectorscope",
            ScopeKind::Histogram => "Histogram",
        }
    }
}

/// The vectorscope picture spans ±this many Cb and Cr units across and up: the centre is grey, and
/// the 100 % colour-bar targets of Rec. 709 fall inside.
pub const VECTORSCOPE_EXTENT: f32 = 0.6;

/// Transparent columns between the parade's R, G and B panes, in the parade picture's pixels.
pub const PARADE_GAP: u32 = 8;

/// Draws scopes. Pure picture analysis: it holds nothing of the project, and any thread may call it.
pub trait Scopes: Send + Sync {
    /// `kind` for `frame` (8-bit RGBA as a viewer shows it), as premultiplied 8-bit RGBA,
    /// transparent where nothing lands:
    ///
    /// - waveform and parade: level 0 % along the bottom row to 100 % along the top; the parade's
    ///   R, G and B panes are equal and side by side, [`PARADE_GAP`] apart;
    /// - vectorscope: square, grey at the centre, +Cr up, spanning ±[`VECTORSCOPE_EXTENT`];
    /// - histogram: level 0 % in the left column to 100 % in the right, counts up from the bottom.
    fn draw(&self, frame: &Frame, kind: ScopeKind) -> Frame;
}
