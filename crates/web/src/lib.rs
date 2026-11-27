//! Chromium (CEF) in off-screen-rendering mode. Each card owns one windowless
//! browser whose frames arrive as BGRA pixel buffers; the app uploads them to
//! GPU textures. All CEF callbacks run on the main thread because we drive CEF
//! with `external_message_pump` + [`pump`], so plain `Rc`/`RefCell` are enough.
//! The one exception is request blocking ([`shield`]), asked on Chromium's IO thread.

mod blocking;
mod engine;
mod events;
mod page;
pub mod prefs;

// The public API stays flat: `web::Page`, `web::WebEvent`, `web::caret`, `web::shield`….
pub use blocking::{consent, cosmetic, shield, youtube};
pub(crate) use blocking::resources;
pub use engine::frame::*;
pub use engine::runtime::*;
pub use engine::start::*;
pub use engine::{keys, popup, render_node, sandbox};
pub(crate) use engine::handlers::*;
pub(crate) use engine::shared::*;
pub use events::*;
pub use page::*;


