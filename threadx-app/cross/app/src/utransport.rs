use crate::uprotocol_v1::{UMessage, UStatus, UUri};
extern crate alloc;

#[trait_variant::make(UTransport: Send)]
pub trait LocalUTransport {
    /// Sends a message using this transport's message exchange mechanism.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to send. The `type`, `source` and `sink` properties of the
    ///   [UAttributes](https://github.com/eclipse-uprotocol/up-spec/blob/v1.6.0-alpha.4/basics/uattributes.adoc) contained
    ///   in the message determine the addressing semantics.
    ///
    /// # Errors
    ///
    /// Returns an error if the message could not be sent.
    async fn send(&self, message: UMessage) -> Result<(), UStatus>;

    // No listener / subscriptions supported.
}

