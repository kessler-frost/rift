//! Inline history menu for up-arrow history.
mod data_source;
mod search_item;
mod view;

pub use data_source::{AcceptHistoryItem, InlineHistoryMenuDataSource};
pub use view::{HistoryTab, InlineHistoryMenuEvent, InlineHistoryMenuView};
