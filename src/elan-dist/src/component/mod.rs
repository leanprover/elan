/// An interpreter for the lean-installer [1] installation format.
///
/// https://github.com/rust-lang/rust-installer
pub use self::package::*;

// The representation of a package, its components, and installation
mod package;
