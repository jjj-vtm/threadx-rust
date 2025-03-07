use embedded_nal::TcpClientStack;
use minimq::{embedded_time, types::Utf8String, Minimq, Property, Publication};

use crate::{uprotocol_v1::{UMessage, UStatus}, utransport::LocalUTransport};

const KEY_UPROTOCOL_VERSION: &str = "uP";
const KEY_MESSAGE_ID: &str = "1";
const KEY_TYPE: &str = "2";
const KEY_SOURCE: &str = "3";
const KEY_SINK: &str = "4";
const KEY_PRIORITY: &str = "5";
const KEY_PERMISSION_LEVEL: &str = "7";
const KEY_COMMSTATUS: &str = "8";
const KEY_TOKEN: &str = "10";
const KEY_TRACEPARENT: &str = "11";

pub struct MiniMqBasedTransport<
    'buf,
    TcpStack: TcpClientStack,
    Clock: embedded_time::Clock,
    Broker: minimq::Broker,
> {
    mqtt_client: Minimq<'buf, TcpStack, Clock, Broker>,
}

impl<'buf, TcpStack: TcpClientStack, Clock: embedded_time::Clock, Broker: minimq::Broker>
    MiniMqBasedTransport<'buf, TcpStack, Clock, Broker>
{
    pub fn new(client: Minimq<'buf, TcpStack, Clock, Broker>) -> Self {
        MiniMqBasedTransport {
            mqtt_client: client,
        }
    }

    pub fn poll(&mut self) {
        match self
            .mqtt_client
            .poll(|_client, _topic, _payload, _properties| 1)
        {
            Ok(_) => (),
            Err(minimq::Error::Network(_)) => {
                defmt::println!("Network disconnect, trying to reconnect.")
            }
            Err(minimq::Error::SessionReset) => {
                defmt::println!("Session reset.")
            }
            _ => panic!("Error during poll, giving up."),
        }
    }

    pub fn is_connected(&mut self) -> bool {
        self.mqtt_client.client().is_connected()
    }
}

impl<TcpStack, Clock, Broker> LocalUTransport for MiniMqBasedTransport<'_, TcpStack, Clock, Broker>
where
    TcpStack: TcpClientStack,
    Clock: embedded_time::Clock,
    Broker: minimq::Broker,
{
    #[doc = " Sends a message using this transport\'s message exchange mechanism."]
    #[doc = ""]
    #[doc = " # Arguments"]
    #[doc = ""]
    #[doc = " * `message` - The message to send. The `type`, `source` and `sink` properties of the"]
    #[doc = "   [UAttributes](https://github.com/eclipse-uprotocol/up-spec/blob/v1.6.0-alpha.4/basics/uattributes.adoc) contained"]
    #[doc = "   in the message determine the addressing semantics."]
    #[doc = ""]
    #[doc = " # Errors"]
    #[doc = ""]
    #[doc = " Returns an error if the message could not be sent."]
    async fn send(&mut self, message: UMessage) -> Result<(), UStatus> {
        let uuid = uuid::uuid!("01956d55-177b-7556-baf6-040e3127165e");
        let buffer = &mut uuid::Uuid::encode_buffer();
        let uuid_hyp = uuid.as_hyphenated().encode_lower(buffer);

        let user_properties = [
            Property::UserProperty(Utf8String(KEY_UPROTOCOL_VERSION), Utf8String("1")),
            // UUID handling
            Property::UserProperty(Utf8String(KEY_MESSAGE_ID), Utf8String(uuid_hyp)),
            Property::UserProperty(Utf8String(KEY_TYPE), Utf8String("up-pub.v1")),
            Property::UserProperty(
                Utf8String(KEY_SOURCE),
                Utf8String("//vehicle_B/000A/2/800A"),
            ),
        ];

        let _ = self
            .mqtt_client
            .client()
            .publish(
                Publication::new("Vehicle_B/000A/0/2/800A", message.payload())
                    .properties(&user_properties),
            )
            .unwrap();
        Ok(())
    }
}
