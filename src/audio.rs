//! Small, platform-neutral audio event helpers.
//! Actual playback remains in the plugin UI so the monitoring engine never
//! depends on an operating-system audio API.

pub const MAX_SOUND_FILE_BYTES: u64 = 2 * 1024 * 1024;
pub const MAX_TOTAL_SOUND_BYTES: u64 = 6 * 1024 * 1024;
pub const MAX_SOUND_DURATION_MS: u64 = 10_000;

pub fn custom_sound_or_beep(has_sound: bool) -> &'static str {
    if has_sound { "custom" } else { "beep" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_sound_uses_beep() {
        assert_eq!(custom_sound_or_beep(false), "beep");
    }

    #[test]
    fn configured_sound_is_custom() {
        assert_eq!(custom_sound_or_beep(true), "custom");
    }

    #[test]
    fn sound_limits_are_bounded() {
        assert_eq!(MAX_SOUND_FILE_BYTES, 2 * 1024 * 1024);
        assert_eq!(MAX_TOTAL_SOUND_BYTES, 6 * 1024 * 1024);
        assert_eq!(MAX_SOUND_DURATION_MS, 10_000);
    }
}
