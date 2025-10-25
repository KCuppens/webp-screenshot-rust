//! Pipeline modules for optimized capture and encoding

pub mod streaming;
pub mod zero_copy;

pub use streaming::{StreamingPipeline, StreamingPipelineBuilder};
pub use zero_copy::ZeroCopyOptimizer;