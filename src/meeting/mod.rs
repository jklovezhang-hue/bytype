//! 会议纪要子系统(与听写循环解耦)。M1:采集 + 混音 + MP3 存档。
pub mod mode;
pub mod naming;

pub use mode::{ModeBehavior, record_behavior};
pub use naming::meeting_base_name;
