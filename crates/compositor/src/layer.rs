//! What the app hands the renderer: a background and layers of quads, page
//! textures and text.

use scene::Rect;

pub type Rgba = [f32; 4];

pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Rgba {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a]
}

/// Background parameters for one frame.
#[derive(Clone, Copy)]
pub struct Background {
    /// The board's colour at its top left; it eases into `color2` towards
    /// the bottom right (the same colour twice: flat).
    pub color: Rgba,
    pub color2: Rgba,
    /// Fine noise over the colour, in 8-bit levels either way (0: none);
    /// it keeps a soft gradient from banding.
    pub grain: f32,
    pub dot: Rgba,
    /// Screen position of world origin, px.
    pub grid_origin: [f32; 2],
    /// Distance between dots, px (dots hidden below ~4px).
    pub grid_spacing: f32,
    pub dot_radius: f32,
    /// Opacity (0–1) of the dots halfway between the main ones: they fade in
    /// as the board zooms in, so the grid never jumps from one density to the next.
    pub fine: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum Shape {
    /// Rounded rectangle in screen px.
    Rect { rect: Rect, radius: f32 },
    /// Rectangle with only its bottom corners rounded (page content under a title bar).
    RoundBottom { rect: Rect, radius: f32 },
    /// Soft shadow under a rounded rectangle.
    Shadow { rect: Rect, radius: f32, blur: f32 },
    /// The outline of a rounded rectangle, `width` px wide, drawn inside `rect`.
    Ring { rect: Rect, radius: f32, width: f32 },
    /// Thick line segment in screen px.
    Segment { from: [f32; 2], to: [f32; 2], width: f32 },
    /// A page texture shown at its own size from the top-left: `extent` is
    /// how many of the texture's widths and heights the rect spans (1, 1
    /// fills it exactly, like `Rect`). Past the texture (extent > 1) the
    /// page's left-edge colour of that row fills in; below 1 it is cropped.
    /// `square`: the corners left square, a mask of [`corners`] (0: all round).
    Page { rect: Rect, radius: f32, square: u8, extent: [f32; 2] },
}

/// Corners of a [`Shape::Page`], to leave square.
pub mod corners {
    pub const TOP_LEFT: u8 = 1;
    pub const TOP_RIGHT: u8 = 2;
    pub const BOTTOM_RIGHT: u8 = 4;
    pub const BOTTOM_LEFT: u8 = 8;
    pub const TOP: u8 = TOP_LEFT | TOP_RIGHT;
    pub const BOTTOM: u8 = BOTTOM_LEFT | BOTTOM_RIGHT;
    pub const LEFT: u8 = TOP_LEFT | BOTTOM_LEFT;
    pub const RIGHT: u8 = TOP_RIGHT | BOTTOM_RIGHT;
}

#[derive(Clone, Copy, Debug)]
pub struct Quad {
    pub shape: Shape,
    pub color: Rgba,
    /// Page texture to fill a `Rect` with (multiplied by `color`).
    pub texture: Option<u64>,
}

/// Shaping width for a single line: wide enough that nothing wraps.
pub(crate) const LINE: f32 = 100_000.0;

#[derive(Clone, Debug)]
pub struct Text {
    pub text: String,
    /// Top-left, screen px.
    pub pos: [f32; 2],
    pub size: f32,
    pub color: Rgba,
    /// Clip rectangle, screen px.
    pub clip: Rect,
    pub bold: bool,
}

#[derive(Default, Clone)]
pub struct Layer {
    pub quads: Vec<Quad>,
    pub texts: Vec<Text>,
}
