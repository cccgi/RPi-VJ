//! Every shipped device profile must parse into a `SurfaceModel`.
//!
//! This is the test that keeps the profile schema and the loader honest: a
//! profile that no longer parses is a broken controller at a gig.

use std::path::Path;
use vj_devices::load_model;
use vj_surface::{CcMode, ControlKind, FeedbackCapability, InputSpec};

fn profiles_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../profiles/models")
}

#[test]
fn every_shipped_profile_parses() {
    let dir = profiles_dir();
    let mut seen = 0;
    for entry in std::fs::read_dir(&dir).expect("profiles/models must exist") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap();
        let model = load_model(&src)
            .unwrap_or_else(|e| panic!("{} failed to parse: {e}", path.display()));
        assert!(!model.controls.is_empty(), "{} declares no controls", path.display());
        seen += 1;
    }
    assert!(seen > 0, "no profiles found in {}", dir.display());
}

#[test]
fn x_touch_encoders_are_relative_with_rings() {
    let src = std::fs::read_to_string(profiles_dir().join("behringer.x-touch-mini.toml")).unwrap();
    let model = load_model(&src).unwrap();

    let enc = model.control(&"enc.0".into()).expect("enc.0 must exist");
    assert!(matches!(enc.kind, ControlKind::Encoder { .. }));

    // Relative encoders need no soft takeover. If this ever reads Absolute,
    // the unit was reset and needs reconfiguring in X-Touch Editor.
    match enc.input {
        InputSpec::Cc { mode, .. } => assert!(mode.is_relative(), "encoders must be relative"),
        other => panic!("expected a CC encoder, got {other:?}"),
    }

    match &enc.feedback {
        FeedbackCapability::EncoderRing { segments, modes } => {
            assert_eq!(*segments, 13);
            assert!(modes.len() >= 4, "ring display modes make bipolar params readable");
        }
        other => panic!("expected an encoder ring, got {other:?}"),
    }

    // The fader is absolute, so it is the one control that will need takeover.
    let fader = model.control(&"fader.0".into()).unwrap();
    assert!(matches!(fader.input, InputSpec::Cc { mode: CcMode::Absolute, .. }));
}
