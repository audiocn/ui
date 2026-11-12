//! The shadcn/ui primitives audiocn's components and blocks are built from
//! (`components/ui/{button,select,popover,...}.tsx`), drawn with GPUI and the
//! audiocn [`Theme`](crate::theme::Theme). Same names and variants as shadcn.

mod alert;
pub use alert::*;
mod badge;
pub use badge::*;
mod button;
pub use button::*;
mod card;
pub use card::*;
mod context_menu;
pub use context_menu::*;
mod dropdown_menu;
pub use dropdown_menu::*;
mod empty;
pub use empty::*;
mod field;
pub use field::*;
mod input;
pub use input::*;
mod kbd;
pub use kbd::*;
mod label;
pub use label::*;
mod popover;
pub use popover::*;
mod select;
pub use select::*;
mod separator;
pub use separator::*;
mod skeleton;
pub use skeleton::*;
mod sonner;
pub use sonner::*;
mod switch;
pub use switch::*;
mod tabs;
pub use tabs::*;
mod toggle;
pub use toggle::*;
mod tooltip;
pub use tooltip::*;

pub mod icons;
mod menu;
pub use menu::*;
mod overlay;
pub use overlay::{Align, Side};
pub(crate) mod styles;

/// Registers key bindings for the primitives (menus, selects, popovers).
pub(crate) fn init(cx: &mut gpui::App) {
    input::init(cx);
}
