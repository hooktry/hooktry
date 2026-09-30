use std::sync::{Arc, RwLock};
use crate::domain::Interaction;

#[derive(Clone, Default)]
pub struct InteractionStore { inner: Arc<RwLock<Vec<Interaction>>> }

impl InteractionStore {
    pub fn record(&self, interaction: Interaction) { self.inner.write().expect("interaction store poisoned").push(interaction); }
    pub fn all(&self) -> Vec<Interaction> { self.inner.read().expect("interaction store poisoned").clone() }
}
