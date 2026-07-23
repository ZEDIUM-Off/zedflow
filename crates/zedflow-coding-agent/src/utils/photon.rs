//! Native equivalent of Pi's optional Photon loader.
//!
//! Rust image utilities use the native `image` crate, so there is no WASM
//! module to patch or load.  Keeping a small handle preserves the optional
//! backend boundary without pulling Photon into the Rust build.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Photon;

/// Return the native image backend when it is available.
pub fn load_photon() -> Option<Photon> {
    Some(Photon)
}
