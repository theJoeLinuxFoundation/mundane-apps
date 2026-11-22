//! Desktop facilities shared by mundaneDE applications.

#[cfg(feature = "terminal")]
pub mod terminal;

#[cfg(feature = "files")]
pub mod files;

#[cfg(feature = "monitor")]
pub mod processes;

#[cfg(feature = "video")]
pub mod playback;

#[cfg(feature = "packages")]
pub mod packages;

#[cfg(feature = "archive")]
pub mod archive;

#[cfg(feature = "usage")]
pub mod usage;
