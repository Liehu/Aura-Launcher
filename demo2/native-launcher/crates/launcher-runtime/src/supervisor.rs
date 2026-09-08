//! ProcessSupervisor (review 52 §1 / 53 §24): the tiny front door to the
//! runtime — spawn + bookkeeping of live RuntimeIds. It deliberately does
//! NOT become an execution manager: no capabilities, no providers, no
//! workflows — just `RuntimeId → live session` mechanics.

use crate::error::RuntimeError;
use crate::process::ProcessSession;
use crate::spec::{LaunchSpec, RuntimeLimits};
use crate::types::RuntimeId;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct ProcessSupervisor {
    live: Arc<Mutex<Vec<RuntimeId>>>,
}

impl ProcessSupervisor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Spawn one external process under `limits`. The returned session owns
    /// the process; dropping it kills the tree (fail-safe containment).
    pub fn spawn(
        &self,
        spec: &LaunchSpec,
        limits: &RuntimeLimits,
    ) -> Result<(RuntimeId, ProcessSession), RuntimeError> {
        let session = ProcessSession::spawn(spec, limits)?;
        let id = session.id.clone();
        self.live.lock().unwrap().push(id.clone());
        Ok((id, session))
    }

    /// Currently-live runtime ids (observability only — not authority).
    pub fn live_ids(&self) -> Vec<RuntimeId> {
        self.live.lock().unwrap().clone()
    }

    pub fn live_count(&self) -> usize {
        self.live.lock().unwrap().len()
    }
}
