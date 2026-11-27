//! GPU compositor for the board: a dot grid, then z-ordered layers. Each layer
//! draws its quads (cards, shadows, page textures, edges) and then its text,
//! so a card's title is covered by cards stacked above it.

mod damage;
mod layer;
mod renderer;

pub use layer::*;
pub use renderer::*;
pub(crate) use damage::*;
