//! Desktop facilities shared by mundaneDE applications.

#[cfg(feature = "terminal")]
pub mod terminal;

#[cfg(feature = "files")]
pub mod files;

#[cfg(feature = "monitor")]
pub mod processes;
