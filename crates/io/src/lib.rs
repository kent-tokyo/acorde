mod error;
#[cfg(feature = "musicxml")]
pub mod musicxml;
#[cfg(feature = "midi")]
pub mod midi;
#[cfg(feature = "abc")]
pub mod abc;
#[cfg(feature = "mscz")]
pub mod mscz;

pub use error::Error;

#[cfg(feature = "musicxml")]
pub use musicxml::{parse_musicxml, parse_mxl, serialize_musicxml};

#[cfg(feature = "midi")]
pub use midi::{parse_midi, serialize_midi, serialize_midi_region};

#[cfg(feature = "abc")]
pub use abc::{parse_abc, serialize_abc};

#[cfg(feature = "mscz")]
pub use mscz::{parse_mscx, parse_mscz};
