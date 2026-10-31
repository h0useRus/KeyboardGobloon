//! The typing engine. It sees the keyboard as physical keys ([`Stroke`]), so the word typed last
//! can be shown in any layout, and answers each [`Input`] with [`Command`]s for the platform to
//! carry out. No OS code here: layouts come in through the [`Layouts`] trait.

mod engine;
#[cfg(test)]
mod fake;
mod keys;
mod text;
mod timings;

pub use engine::{Command, Engine, Input, Layouts, next_layout};
pub use kbg_settings::Action;
pub use keys::{Key, LayoutId, Stroke};
pub use text::{convert_text, cycle_case};
pub use timings::Timings;
