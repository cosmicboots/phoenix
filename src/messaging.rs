//! Messaging module to handle conversion to and from protocol message structure.
//!
//! This module defines the basic structure for messages between the client and server.
//! It also implements the `From` traits between `Vec<u8>`/`&[u8]` and
//! [`Message`](struct.Message.html).
//!
//! Conversion between the two could look like this:
//! ```rust
//! let mut msg: Message = Message {
//!     id: 0,
//!     verb: Directive::SendFile,
//!     data: Some(Data::new(&vec![1, 2, 3])),
//! };
//! assert_eq!(Vec::from(msg), vec!(0, 0, 0, 3, 0, 0, 0, 3, 1, 2, 3),);
//! ```
//!
//! And back:
//! ```
//! let mut msg: &[u8] = &[0u8, 0u8, 0u8, 3u8, 0u8, 0u8, 0u8, 3u8, 1u8, 2u8, 3u8][..];
//! assert_eq!(
//!     Message::from(msg),
//!     Message {
//!         id: 0,
//!         verb: Directive::SendFile,
//!         data: Some(Data::new(&vec![1, 2, 3])),
//!     }
//! );
//! ```

#![allow(dead_code)]
#![allow(unused_variables)]

struct Version(u8);

/// Defines the different available protocol verbs/directives.
#[derive(Debug, PartialEq)]
pub enum Directive {
    ListFiles,
    RequestFile,
    RequestChunk,
    SendFile,
    SendChunk,
    DeleteFile,
    Response,
}

/// Covert from u16 to Directive.
/// This should proably be handled better in the future
impl TryFrom<u16> for Directive {
    type Error = &'static str;
    fn try_from(num: u16) -> Result<Self, Self::Error> {
        // TODO: This logic seems verbose
        match num {
            0 => Ok(Directive::ListFiles),
            1 => Ok(Directive::RequestFile),
            2 => Ok(Directive::RequestChunk),
            3 => Ok(Directive::SendFile),
            4 => Ok(Directive::SendChunk),
            5 => Ok(Directive::DeleteFile),
            6 => Ok(Directive::Response),
            _ => Err("Failed to convert Directive"),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Data {
    size: u32,
    data: Vec<u8>,
}

impl Data {
    fn new(data: &[u8]) -> Data {
        Data {
            size: data.len() as u32,
            data: data.to_vec(),
        }
    }
}

/// This is the main structure for every message sent over the network.
#[derive(PartialEq, Debug)]
pub struct Message {
    id: u16,
    verb: Directive,
    data: Option<Data>,
}

impl From<Message> for Vec<u8> {
    fn from(msg: Message) -> Vec<u8> {
        let mut buffer: Vec<u8> = vec![];

        // Add the message id
        buffer.extend(msg.id.to_be_bytes());

        // Add the directive
        buffer.extend((msg.verb as u16).to_be_bytes());

        // Add the data
        if let Some(d) = msg.data {
            // Add the size
            buffer.extend(d.size.to_be_bytes());
            // Add the data
            buffer.extend(d.data);
        }
        buffer.to_owned()
    }
}

impl From<&[u8]> for Message {
    fn from(msg: &[u8]) -> Message {
        // Two byte buffer will be used to create be arrays
        let mut buf: [u8; 2] = [0u8; 2];

        // Deserialize message id
        buf.copy_from_slice(&msg[0..2]);
        let id: u16 = u16::from_be_bytes(buf);

        // Deserialize the derective
        buf.copy_from_slice(&msg[2..4]);
        let verb: Directive = match u16::from_be_bytes(buf).try_into() {
            Ok(x) => x,
            Err(x) => panic!("{}", x),
        };

        // Add the data
        let data: Option<Data> = match msg.len() {
            0..=8 => None,
            _ => Some(Data::new(&msg[8..])),
        };

        Message { id, verb, data }
    }
}

/// Build messages according to the <insert_protocol_name_here> protocol.
///
/// A MessageBuilder will be created for each connection. It's main goal is to keep track of the
/// current MessageId and encode/decode message packets
pub struct MessageBuilder {
    protocol_version: Version,
    current_request: u16,
}

impl MessageBuilder {
    pub fn new(ver: u8) -> MessageBuilder {
        return MessageBuilder {
            protocol_version: Version(ver),
            current_request: 0,
        };
    }

    pub fn encode_message(id: u16, verb: Directive, data: Option<Data>) -> Vec<u8> {
        let buffer: Vec<u8> = vec![];
        let msg = Message { id, verb, data };

        todo!();
    }

    pub fn decode_message(message: Vec<u8>) -> Message {
        todo!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msg_ser() {
        let mut msg: Message = Message {
            id: 0,
            verb: Directive::SendFile,
            data: Some(Data::new(&vec![1, 2, 3])),
        };
        assert_eq!(Vec::from(msg), vec!(0, 0, 0, 3, 0, 0, 0, 3, 1, 2, 3),);
        msg = Message {
            id: 1,
            verb: Directive::ListFiles,
            data: None,
        };
        assert_eq!(Vec::from(msg), vec!(0, 1, 0, 0));
    }

    #[test]
    fn test_msg_de() {
        let mut msg: &[u8] = &[0u8, 0u8, 0u8, 3u8, 0u8, 0u8, 0u8, 3u8, 1u8, 2u8, 3u8][..];
        assert_eq!(
            Message::from(msg),
            Message {
                id: 0,
                verb: Directive::SendFile,
                data: Some(Data::new(&vec![1, 2, 3])),
            }
        );
        msg = &[0u8, 1u8, 0u8, 0u8];
        assert_eq!(
            Message::from(msg),
            Message {
                id: 1,
                verb: Directive::ListFiles,
                data: None,
            }
        );
    }
}
