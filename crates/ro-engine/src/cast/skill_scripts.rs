//! The skill definitions plus the hook backend their ids belong to.

use std::fmt;
use std::sync::Arc;

use ro_scripting::{Definitions, HookBackend, MissingHook, RustHooks};

/// The skill definitions and the hook backend their hook ids belong to (spec §2.1). They are
/// built together, so the ids always match the backend.
#[derive(Clone, Default)]
pub struct SkillScripts {
    /// `None`: nothing can be cast (the default, for maps and tests that don't need skills).
    definitions: Option<Arc<Definitions>>,
    hooks: Option<Arc<dyn HookBackend + Send + Sync>>,
}

impl SkillScripts {
    /// The built-in definitions bound to `hooks`.
    pub fn with_backend(hooks: Arc<dyn HookBackend + Send + Sync>) -> Result<Self, MissingHook> {
        let definitions = Definitions::builtin(&*hooks)?;
        Ok(Self {
            definitions: Some(Arc::new(definitions)),
            hooks: Some(hooks),
        })
    }

    /// The built-in definitions with the Rust hooks.
    pub fn builtin() -> Result<Self, MissingHook> {
        Self::with_backend(Arc::new(RustHooks::builtin()))
    }

    pub(crate) fn get(&self) -> Option<(Arc<Definitions>, Arc<dyn HookBackend + Send + Sync>)> {
        Some((
            Arc::clone(self.definitions.as_ref()?),
            Arc::clone(self.hooks.as_ref()?),
        ))
    }
}

impl fmt::Debug for SkillScripts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SkillScripts")
            .field("definitions", &self.definitions)
            .field("hooks", &self.hooks.as_ref().map(|_| "HookBackend"))
            .finish()
    }
}
