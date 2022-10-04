//! Messaging module to handle conversion to and from protocol message structure.
//!
//! This module defines the basic structure for messages between the client and server.
//! It also implements the `From` traits between `Vec<u8>`/`&[u8]` and
//! [`Message`](struct.Message.html).
//!
//! ## Design
//!
//! The network protocol follows a very simple messaging structure.
//!
//! Each message sent over the network should be encoded in binary and structured as follows:
//!
//! ```
//! <msg-num:u16> <verb:u16> [<argument>]
//! ```
//!
//! - `msg-num` is a 16-bit unsigned integer that represents each network packet with a unique
//! number.
//! - `verb` is a 16-bit unsigned integer that represents an action to be taken on the responders
//! part. This can be thought of as a command/directive/verb.
//! - `argument` completely depends on the `verb`. Each `verb` will have its own argument type, and
//! each argument can define its own structure. As such, arguments can be fixed or dynamic in size.
//!
//! A list of arguments can be found in the [`arguments`](arguments/index.html) sub-module.
//!
//! Verbs/directives are defined in the [`Directive`](enum.Directive.html) enum.
//!
//! ## Examples
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

pub mod arguments;

use arguments::{Argument, Version};

//struct Version(u8);

/// Defines the different available protocol verbs/directives.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Directive {
    AnnounceVersion,
    ListFiles,
    SendFiles,
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
            0 => Ok(Directive::AnnounceVersion),
            1 => Ok(Directive::ListFiles),
            2 => Ok(Directive::SendFiles),
            3 => Ok(Directive::RequestFile),
            4 => Ok(Directive::RequestChunk),
            5 => Ok(Directive::SendFile),
            6 => Ok(Directive::SendChunk),
            7 => Ok(Directive::DeleteFile),
            8 => Ok(Directive::Response),
            _ => Err("Failed to convert Directive"),
        }
    }
}

/// This is the main structure for every message sent over the network.
#[derive(Debug)]
pub struct Message {
    pub id: u16,
    pub verb: Directive,
    pub argument: Option<Box<dyn Argument>>,
}

#[derive(PartialEq, Debug)]
struct RawMessage {
    id: u16,
    verb: Directive,
    data: Option<Vec<u8>>,
}

impl From<RawMessage> for Vec<u8> {
    fn from(msg: RawMessage) -> Vec<u8> {
        let mut buffer: Vec<u8> = vec![];

        // Add the message id
        buffer.extend(msg.id.to_be_bytes());

        // Add the directive
        buffer.extend((msg.verb as u16).to_be_bytes());

        // Add the data
        if let Some(d) = msg.data {
            // Add the data
            buffer.extend(d);
        }
        buffer.to_owned()
    }
}

impl From<&[u8]> for RawMessage {
    fn from(msg: &[u8]) -> RawMessage {
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
        let data: Option<Vec<u8>> = match msg.len() {
            0..=4 => None,
            _ => Some(msg[4..].to_vec()),
        };

        RawMessage { id, verb, data }
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
        MessageBuilder {
            protocol_version: Version(ver),
            current_request: 0,
        }
    }

    /// Encode a message from language constructs to a binary packet format
    pub fn encode_message<T>(&mut self, verb: Directive, argument: Option<T>) -> Vec<u8>
    where
        T: Argument,
    {
        // Encode message arguments
        let encoded_data = argument.map(|x| x.to_bin());

        let msg = RawMessage {
            id: self.current_request,
            verb,
            data: encoded_data,
        };

        self.current_request += 1;

        msg.into()
    }

    pub fn decode_message(message: &[u8]) -> Result<Box<Message>, arguments::Error> {
        let msg = RawMessage::from(message);

        let mut arg: Option<Box<dyn Argument>> = None;

        if let Some(x) = msg.data {
            // It's best to keep this match verbose. If directives are added in the future, the
            // exhaustive match will force us to handle its argument type here.
            arg = match msg.verb {
                Directive::AnnounceVersion => Some(Box::new(arguments::Version::from_bin(&x)?)),
                Directive::ListFiles => None,
                Directive::SendFiles => Some(Box::new(arguments::FileList::from_bin(&x)?)),
                Directive::RequestFile => Some(Box::new(arguments::FileId::from_bin(&x)?)),
                Directive::RequestChunk => {
                    Some(Box::new(arguments::QualifiedChunkId::from_bin(&x)?))
                }
                Directive::SendFile => Some(Box::new(arguments::FileMetadata::from_bin(&x)?)),
                Directive::SendChunk => Some(Box::new(arguments::Chunk::from_bin(&x)?)),
                Directive::DeleteFile => Some(Box::new(arguments::FileId::from_bin(&x)?)),
                Directive::Response => Some(Box::new(arguments::ResponseCode::from_bin(&x)?)),
            };
        }

        Ok(Box::new(Message {
            id: msg.id,
            verb: msg.verb,
            argument: arg,
        }))
    }

    pub fn increment_counter(&mut self) {
        self.current_request += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msg_ser() {
        let mut msg: RawMessage = RawMessage {
            id: 0,
            verb: Directive::SendFile,
            data: Some(vec![1, 2, 3]),
        };
        assert_eq!(Vec::from(msg), vec!(0, 0, 0, 5, 1, 2, 3),);
        msg = RawMessage {
            id: 1,
            verb: Directive::ListFiles,
            data: None,
        };
        assert_eq!(Vec::from(msg), vec!(0, 1, 0, 1));
    }

    #[test]
    fn test_msg_de() {
        let mut msg_raw: &[u8] = &[0u8, 0u8, 0u8, 0u8, 1u8][..];
        let mut msg = RawMessage {
            id: 0,
            verb: Directive::AnnounceVersion,
            data: Some(vec![1]),
        };
        assert_eq!(RawMessage::from(msg_raw), msg,);
        msg_raw = &[1u8, 0u8, 0u8, 0u8, 1u8];
        msg.id += 256;
        assert_eq!(RawMessage::from(msg_raw), msg,);
    }
}
