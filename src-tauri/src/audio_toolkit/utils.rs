use rodio::cpal;

/// The CPAL host feedback sounds play through. Microphone capture picks its
/// own host inside handy-recorder.
pub fn get_cpal_host() -> cpal::Host {
    cpal::default_host()
}
