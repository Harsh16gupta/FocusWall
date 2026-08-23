//! Domain normalization and list definitions

/// Canonical list of YouTube domains that must be blocked.
pub const YOUTUBE_DOMAINS: &[&str] = &[
    "youtube.com",
    "www.youtube.com",
    "m.youtube.com",
    "music.youtube.com",
    "youtu.be",
    "youtube-nocookie.com",
    "ytimg.com",
    "googlevideo.com",
];
