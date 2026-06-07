#![cfg_attr(target_family = "wasm", allow(dead_code, unused_imports))]

pub(super) mod diff;
mod element;
pub mod find;
pub mod goto_line;
pub mod line;
mod line_iterator;
pub mod model;
mod nav_bar;
pub mod scroll;
pub mod view;

pub(crate) use diff::{add_color, remove_color};
pub use element::GutterHoverTarget;
pub use nav_bar::NavBarBehavior;
