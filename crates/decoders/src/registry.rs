//! The registry of decoders, keyed by descriptor id.
//!
//! Registering a descriptor is all a decoder needs to appear in the list, the configuration dialog,
//! the lane and the saved setup. Registration validates the descriptor and refuses an invalid one
//! (a developer error that is logged and never reaches the user as a crash).

use la_core::Error;

use crate::descriptor::Descriptor;
use crate::error;

/// The decoders the application knows.
#[derive(Debug, Default, Clone)]
pub struct Registry {
    entries: Vec<&'static Descriptor>,
}

impl Registry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The registry of the built-in decoders.
    #[must_use]
    pub fn builtin() -> Self {
        Self::new()
    }

    /// Adds a decoder.
    ///
    /// # Errors
    ///
    /// The descriptor is invalid or its id is already registered; the registry is unchanged.
    pub fn register(&mut self, descriptor: &'static Descriptor) -> Result<(), Error> {
        if let Err(reason) = descriptor.validate() {
            tracing::error!(decoder = descriptor.id, %reason, "refused an invalid decoder descriptor");
            return Err(error::failed(format!(
                "invalid descriptor of `{}`: {reason}",
                descriptor.id
            )));
        }
        if self.get(descriptor.id).is_some() {
            return Err(error::failed(format!(
                "decoder `{}` is registered twice",
                descriptor.id
            )));
        }
        self.entries.push(descriptor);
        Ok(())
    }

    /// The decoder with this id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&'static Descriptor> {
        self.entries.iter().copied().find(|d| d.id == id)
    }

    /// Every registered decoder, in registration order.
    #[must_use]
    pub fn list(&self) -> &[&'static Descriptor] {
        &self.entries
    }
}
