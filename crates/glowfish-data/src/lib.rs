//! Data handling for glowfish, independent of any renderer: a lazily loaded min/max mipmap of
//! stream files, pluggable downsampling methods and value scaling.

mod downsample;
mod error;
mod load;
mod pyramid;
mod scaling;
mod source;

pub use downsample::{Downsampler, MinMax, MinMaxCell};
pub use error::DataError;
pub use load::{LoadControl, LoadPool};
pub use pyramid::{Grid, Pyramid};
pub use scaling::{Scaling, ValueRange};
pub use source::Source;
