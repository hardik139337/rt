//! BitTorrent protocol messages
//!
//! Defines all message types used in the BitTorrent protocol.

use bytes::{Buf, BufMut, BytesMut};
use anyhow::Result;
use tracing::{debug, error, trace};

use crate::error::TorrentError;

/// BitTorrent message IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageId {
    Choke = 0,
    Unchoke = 1,
    Interested = 2,
    NotInterested = 3,
    Have = 4,
    Bitfield = 5,
    Request = 6,
    Piece = 7,
    Cancel = 8,
    Port = 9,
}

impl TryFrom<u8> for MessageId {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self> {
        trace!("Converting byte to MessageId: {}", value);
        match value {
            0 => Ok(MessageId::Choke),
            1 => Ok(MessageId::Unchoke),
            2 => Ok(MessageId::Interested),
            3 => Ok(MessageId::NotInterested),
            4 => Ok(MessageId::Have),
            5 => Ok(MessageId::Bitfield),
            6 => Ok(MessageId::Request),
            7 => Ok(MessageId::Piece),
            8 => Ok(MessageId::Cancel),
            9 => Ok(MessageId::Port),
            _ => {
                error!("Invalid message ID: {}", value);
                Err(TorrentError::protocol_error_with_source(
                    "Invalid message ID",
                    format!("value: {}", value)
                ).into())
            }
        }
    }
}

/// BitTorrent protocol message
#[derive(Debug, Clone)]
pub enum Message {
    KeepAlive,
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have { piece_index: u32 },
    Bitfield { bitfield: Vec<u8> },
    Request { index: u32, begin: u32, length: u32 },
    Piece { index: u32, begin: u32, block: Vec<u8> },
    Cancel { index: u32, begin: u32, length: u32 },
    Port { listen_port: u16 },
}

impl Message {
    /// Get the message ID (returns None for KeepAlive)
    pub fn message_id(&self) -> Option<MessageId> {
        match self {
            Message::Choke => Some(MessageId::Choke),
            Message::Unchoke => Some(MessageId::Unchoke),
            Message::Interested => Some(MessageId::Interested),
            Message::NotInterested => Some(MessageId::NotInterested),
            Message::Have { .. } => Some(MessageId::Have),
            Message::Bitfield { .. } => Some(MessageId::Bitfield),
            Message::Request { .. } => Some(MessageId::Request),
            Message::Piece { .. } => Some(MessageId::Piece),
            Message::Cancel { .. } => Some(MessageId::Cancel),
            Message::Port { .. } => Some(MessageId::Port),
            Message::KeepAlive => None,
        }
    }

    /// Get the message length (excluding the length prefix)
    pub fn length(&self) -> u32 {
        match self {
            Message::KeepAlive => 0,
            Message::Choke => 1,
            Message::Unchoke => 1,
            Message::Interested => 1,
            Message::NotInterested => 1,
            Message::Have { .. } => 5,
            Message::Bitfield { bitfield } => 1 + bitfield.len() as u32,
            Message::Request { .. } => 13,
            Message::Piece { block, .. } => 9 + block.len() as u32,
            Message::Cancel { .. } => 13,
            Message::Port { .. } => 3,
        }
    }

    /// Get the block length for Piece messages
    pub fn block_len(&self) -> Option<usize> {
        match self {
            Message::Piece { block, .. } => Some(block.len()),
            _ => None,
        }
    }

    /// Serialize the message to bytes (including length prefix)
    pub fn serialize(&self) -> Vec<u8> {
        trace!("Serializing message: {:?}", self.message_id());
        let mut buf = BytesMut::new();

        // Write length prefix
        buf.put_u32(self.length());

        match self {
            Message::KeepAlive => {
                // No message ID for KeepAlive
            }
            Message::Choke => {
                buf.put_u8(MessageId::Choke as u8);
            }
            Message::Unchoke => {
                buf.put_u8(MessageId::Unchoke as u8);
            }
            Message::Interested => {
                buf.put_u8(MessageId::Interested as u8);
            }
            Message::NotInterested => {
                buf.put_u8(MessageId::NotInterested as u8);
            }
            Message::Have { piece_index } => {
                buf.put_u8(MessageId::Have as u8);
                buf.put_u32(*piece_index);
            }
            Message::Bitfield { bitfield } => {
                buf.put_u8(MessageId::Bitfield as u8);
                buf.put_slice(bitfield);
            }
            Message::Request { index, begin, length } => {
                buf.put_u8(MessageId::Request as u8);
                buf.put_u32(*index);
                buf.put_u32(*begin);
                buf.put_u32(*length);
            }
            Message::Piece { index, begin, block } => {
                buf.put_u8(MessageId::Piece as u8);
                buf.put_u32(*index);
                buf.put_u32(*begin);
                buf.put_slice(block);
            }
            Message::Cancel { index, begin, length } => {
                buf.put_u8(MessageId::Cancel as u8);
                buf.put_u32(*index);
                buf.put_u32(*begin);
                buf.put_u32(*length);
            }
            Message::Port { listen_port } => {
                buf.put_u8(MessageId::Port as u8);
                buf.put_u16(*listen_port);
            }
        }

        trace!("Message serialized: {} bytes", buf.len());
        buf.to_vec()
    }

    /// Deserialize a message from bytes (including length prefix)
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        trace!("Deserializing message from {} bytes", data.len());
        let mut buf = BytesMut::from(data);

        if buf.is_empty() {
            error!("Empty message data");
            return Err(TorrentError::protocol_error("Empty message data").into());
        }

        // Read length prefix
        let length = buf.get_u32() as usize;
        debug!("Message length prefix: {}", length);

        // KeepAlive message has length 0 and no message ID
        if length == 0 {
            debug!("Received KeepAlive message");
            return Ok(Message::KeepAlive);
        }

        // Check if we have enough data for the message ID
        if buf.remaining() < 1 {
            error!("Message too short: missing message ID");
            return Err(TorrentError::protocol_error_with_source(
                "Message too short",
                "missing message ID"
            ).into());
        }

        let id = buf.get_u8();
        let message_id = MessageId::try_from(id)?;
        debug!("Message ID: {:?}", message_id);

        match message_id {
            MessageId::Choke => {
                debug!("Received Choke message");
                Ok(Message::Choke)
            }
            MessageId::Unchoke => {
                debug!("Received Unchoke message");
                Ok(Message::Unchoke)
            }
            MessageId::Interested => {
                debug!("Received Interested message");
                Ok(Message::Interested)
            }
            MessageId::NotInterested => {
                debug!("Received NotInterested message");
                Ok(Message::NotInterested)
            }
            MessageId::Have => {
                if buf.remaining() < 4 {
                    error!("Have message too short: expected 4 bytes, got {}", buf.remaining());
                    return Err(TorrentError::protocol_error_with_source(
                        "Have message too short",
                        format!("expected 4 bytes, got {}", buf.remaining())
                    ).into());
                }
                let piece_index = buf.get_u32();
                debug!("Received Have message for piece {}", piece_index);
                Ok(Message::Have { piece_index })
            }
            MessageId::Bitfield => {
                let bitfield = buf.to_vec();
                debug!("Received Bitfield message with {} bytes", bitfield.len());
                Ok(Message::Bitfield { bitfield })
            }
            MessageId::Request => {
                if buf.remaining() < 12 {
                    error!("Request message too short: expected 12 bytes, got {}", buf.remaining());
                    return Err(TorrentError::protocol_error_with_source(
                        "Request message too short",
                        format!("expected 12 bytes, got {}", buf.remaining())
                    ).into());
                }
                let index = buf.get_u32();
                let begin = buf.get_u32();
                let length = buf.get_u32();
                debug!("Received Request message: index={}, begin={}, length={}", index, begin, length);
                Ok(Message::Request { index, begin, length })
            }
            MessageId::Piece => {
                if buf.remaining() < 8 {
                    error!("Piece message too short: expected at least 8 bytes, got {}", buf.remaining());
                    return Err(TorrentError::protocol_error_with_source(
                        "Piece message too short",
                        format!("expected at least 8 bytes, got {}", buf.remaining())
                    ).into());
                }
                let index = buf.get_u32();
                let begin = buf.get_u32();
                let block = buf.to_vec();
                debug!("Received Piece message: index={}, begin={}, block_len={}", index, begin, block.len());
                Ok(Message::Piece { index, begin, block })
            }
            MessageId::Cancel => {
                if buf.remaining() < 12 {
                    error!("Cancel message too short: expected 12 bytes, got {}", buf.remaining());
                    return Err(TorrentError::protocol_error_with_source(
                        "Cancel message too short",
                        format!("expected 12 bytes, got {}", buf.remaining())
                    ).into());
                }
                let index = buf.get_u32();
                let begin = buf.get_u32();
                let length = buf.get_u32();
                debug!("Received Cancel message: index={}, begin={}, length={}", index, begin, length);
                Ok(Message::Cancel { index, begin, length })
            }
            MessageId::Port => {
                if buf.remaining() < 2 {
                    error!("Port message too short: expected 2 bytes, got {}", buf.remaining());
                    return Err(TorrentError::protocol_error_with_source(
                        "Port message too short",
                        format!("expected 2 bytes, got {}", buf.remaining())
                    ).into());
                }
                let listen_port = buf.get_u16();
                debug!("Received Port message: listen_port={}", listen_port);
                Ok(Message::Port { listen_port })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialize_deserialize_choke() {
        let message = Message::Choke;
        let serialized = message.serialize();
        let deserialized = Message::deserialize(&serialized).unwrap();
        assert_eq!(message.message_id(), deserialized.message_id());
    }

    #[test]
    fn test_message_serialize_deserialize_keepalive() {
        let message = Message::KeepAlive;
        let serialized = message.serialize();
        assert_eq!(serialized, vec![0, 0, 0, 0]);
        let deserialized = Message::deserialize(&serialized).unwrap();
        assert_eq!(message.message_id(), deserialized.message_id());
    }

    #[test]
    fn test_message_serialize_deserialize_have() {
        let message = Message::Have { piece_index: 42 };
        let serialized = message.serialize();
        let deserialized = Message::deserialize(&serialized).unwrap();
        match deserialized {
            Message::Have { piece_index } => assert_eq!(piece_index, 42),
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_message_serialize_deserialize_request() {
        let message = Message::Request { index: 1, begin: 2, length: 3 };
        let serialized = message.serialize();
        let deserialized = Message::deserialize(&serialized).unwrap();
        match deserialized {
            Message::Request { index, begin, length } => {
                assert_eq!(index, 1);
                assert_eq!(begin, 2);
                assert_eq!(length, 3);
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_message_serialize_deserialize_piece() {
        let block = vec![1, 2, 3, 4, 5];
        let message = Message::Piece { index: 10, begin: 0, block: block.clone() };
        let serialized = message.serialize();
        let deserialized = Message::deserialize(&serialized).unwrap();
        match deserialized {
            Message::Piece { index, begin, block } => {
                assert_eq!(index, 10);
                assert_eq!(begin, 0);
                assert_eq!(block, block);
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_message_serialize_deserialize_port() {
        let message = Message::Port { listen_port: 6881 };
        let serialized = message.serialize();
        let deserialized = Message::deserialize(&serialized).unwrap();
        match deserialized {
            Message::Port { listen_port } => assert_eq!(listen_port, 6881),
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_message_length() {
        assert_eq!(Message::KeepAlive.length(), 0);
        assert_eq!(Message::Choke.length(), 1);
        assert_eq!(Message::Have { piece_index: 0 }.length(), 5);
        assert_eq!(Message::Request { index: 0, begin: 0, length: 0 }.length(), 13);
        assert_eq!(Message::Piece { index: 0, begin: 0, block: vec![1, 2, 3] }.length(), 12);
        assert_eq!(Message::Port { listen_port: 0 }.length(), 3);
    }

    #[test]
    fn test_message_id() {
        assert_eq!(Message::Choke.message_id(), Some(MessageId::Choke));
        assert_eq!(Message::Unchoke.message_id(), Some(MessageId::Unchoke));
        assert_eq!(Message::Interested.message_id(), Some(MessageId::Interested));
        assert_eq!(Message::NotInterested.message_id(), Some(MessageId::NotInterested));
        assert_eq!(Message::Have { piece_index: 0 }.message_id(), Some(MessageId::Have));
        assert_eq!(Message::Bitfield { bitfield: vec![] }.message_id(), Some(MessageId::Bitfield));
        assert_eq!(Message::Request { index: 0, begin: 0, length: 0 }.message_id(), Some(MessageId::Request));
        assert_eq!(Message::Piece { index: 0, begin: 0, block: vec![] }.message_id(), Some(MessageId::Piece));
        assert_eq!(Message::Cancel { index: 0, begin: 0, length: 0 }.message_id(), Some(MessageId::Cancel));
        assert_eq!(Message::Port { listen_port: 0 }.message_id(), Some(MessageId::Port));
        assert_eq!(Message::KeepAlive.message_id(), None);
    }

    #[test]
    fn test_message_id_from_u8() {
        assert_eq!(MessageId::try_from(0).unwrap(), MessageId::Choke);
        assert_eq!(MessageId::try_from(1).unwrap(), MessageId::Unchoke);
        assert_eq!(MessageId::try_from(9).unwrap(), MessageId::Port);
        assert!(MessageId::try_from(10).is_err());
    }
}
