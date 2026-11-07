//! `EngineError` display text.

use ro_core::CharId;
use ro_engine::EngineError;

#[test]
fn engine_error_new_variants_display() {
    assert!(EngineError::MapNotFound("x.gat".into())
        .to_string()
        .contains("x.gat"));
    assert!(EngineError::CharacterAlreadyEntered(CharId(7))
        .to_string()
        .contains('7'));
    assert!(!EngineError::WorldCapacityExceeded.to_string().is_empty());
}
