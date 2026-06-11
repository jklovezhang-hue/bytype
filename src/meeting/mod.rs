//! 会议纪要子系统(与听写循环解耦)。M1:采集 + 混音 + MP3 存档。
pub mod mode;
pub mod naming;
pub mod resample;
pub mod mix;
pub mod wav;
pub mod mp3;
pub mod session;
pub mod capture_mic;
pub mod capture_system;
pub mod transcript;

pub use mode::{ModeBehavior, record_behavior};
pub use naming::meeting_base_name;
pub use resample::resample_to_16k;
pub use mix::mix_tracks;
pub use wav::WavSink;
pub use mp3::encode_mp3_file;
pub use session::{MeetingPaths, MeetingSession, plan_retention};
pub use capture_mic::MicCapture;
pub use capture_system::SystemCapture;
pub use transcript::{Line, Speaker, Transcript};
