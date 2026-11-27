//! A card's page and what it asks of the app: editing and the clipboard, find and zoom, dialogs, file chooser, permissions, cookies, the inspector, downloads.

pub mod clipboard;
mod cookies;
pub(crate) mod dialogs;
pub(crate) mod downloads;
mod editing;
pub mod editor;
pub(crate) mod file_chooser;
mod find_zoom;
pub(crate) mod inspector;
pub mod pathfield;
mod permissions;
