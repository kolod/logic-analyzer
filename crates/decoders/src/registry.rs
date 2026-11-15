//! The registry of decoders, keyed by descriptor id.
//!
//! Registering a descriptor is all a decoder needs to appear in the list, the configuration dialog,
//! the lane and the saved setup. Registration validates the descriptor and refuses an invalid one
//! (a developer error that is logged and never reaches the user as a crash).

use la_core::Error;

use crate::descriptor::Descriptor;
use crate::error;
use crate::run::Run;
use crate::setup::Setup;

/// Creates the state of one pass of a decoder from a validated setup.
pub type Create = fn(&Setup) -> Result<Box<dyn Run>, Error>;

/// The decoders the application knows.
#[derive(Debug, Default, Clone)]
pub struct Registry {
    entries: Vec<(&'static Descriptor, Option<Create>)>,
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
        let mut registry = Self::new();
        // A built-in descriptor that fails validation is a bug caught by the tests.
        if let Err(e) = registry.register_decoder(
            &crate::decoders::clocked_bits::DESCRIPTOR,
            crate::decoders::clocked_bits::create,
        ) {
            tracing::error!(code = e.code(), "a built-in decoder was not registered");
        }
        registry
    }

    /// Adds a decoder with the function that creates its passes.
    ///
    /// # Errors
    ///
    /// As [`Registry::register`].
    pub fn register_decoder(
        &mut self,
        descriptor: &'static Descriptor,
        create: Create,
    ) -> Result<(), Error> {
        self.register(descriptor)?;
        if let Some(entry) = self.entries.last_mut() {
            entry.1 = Some(create);
        }
        Ok(())
    }

    /// Creates the state of a pass for a validated setup.
    ///
    /// # Errors
    ///
    /// `decoder.invalid_setup` if the decoder is unknown or has no way to create passes; whatever
    /// the decoder reports otherwise.
    pub fn create(&self, setup: &Setup) -> Result<Box<dyn Run>, Error> {
        let create = self
            .entries
            .iter()
            .find(|(d, _)| d.id == setup.decoder)
            .and_then(|(_, create)| *create)
            .ok_or_else(|| error::invalid_setup("decoder: cannot be run"))?;
        create(setup)
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
        self.entries.push((descriptor, None));
        Ok(())
    }

    /// The decoder with this id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&'static Descriptor> {
        self.entries.iter().map(|(d, _)| *d).find(|d| d.id == id)
    }

    /// Every registered decoder, in registration order.
    #[must_use]
    pub fn list(&self) -> Vec<&'static Descriptor> {
        self.entries.iter().map(|(d, _)| *d).collect()
    }
}
