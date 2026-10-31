//! The typing engine. It sees the keyboard as physical keys ([`Stroke`]), so the word typed last
//! can be shown in any layout, and answers each [`Input`] with [`Command`]s for the platform to
//! carry out. No OS code here: layouts come in through the [`Layouts`] trait.

mod engine;
mod keys;

pub use engine::{Command, Engine, Input, Layouts};
pub use keys::{Key, LayoutId, Stroke};
