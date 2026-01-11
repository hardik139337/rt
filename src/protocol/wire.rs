//! Wire protocol utilities
//!
//! Helper functions and traits for working with the BitTorrent wire protocol.

use bytes::{Buf, BufMut, BytesMut};
use anyhow::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{Handshake, Message};

/// WireProtocol trait for protocol utilities
pub trait WireProtocol {
    /// Read a complete message from the stream
    async fn read_message<R: AsyncReadExt + Unpin>(&mut self, reader: &mut R) -> Result<Message>;

    /// Write a message to the stream
    async fn write_message<W: AsyncWriteExt + Unpin>(&mut self, writer: &mut W, message: &Message) -> Result<()>;

    /// Read a handshake from the stream
    async fn read_handshake<R: AsyncReadExt + Unpin>(&mut self, reader: &mut R) -> Result<Handshake>;

    /// Write a handshake to the stream
    async fn write_handshake<W: AsyncWriteExt + Unpin>(&mut self, writer: &mut W, handshake: &Handshake) -> Result<()>;
}

/// Default implementation of WireProtocol
pub struct BitTorrentWire;

impl WireProtocol for BitTorrentWire {
    /// Read a complete message from the stream
    async fn read_message<R: AsyncReadExt + Unpin>(&mut self, reader: &mut R) -> Result<Message> {
        // Read the length prefix (4 bytes)
        let mut length_buf = [0u8; 4];
        reader.read_exact(&mut length_buf).await?;
        let length = u32::from_be_bytes(length_buf) as usize;

        // KeepAlive message has length 0
        if length == 0 {
            return Ok(Message::KeepAlive);
        }

        // Read the message payload
        let mut payload = vec![0u8; length];
        reader.read_exact(&mut payload).await?;

        // Parse the message
        let mut full_message = BytesMut::with_capacity(4 + length);
        full_message.put_slice(&length_buf);
        full_message.put_slice(&payload);

        Message::deserialize(&full_message)
    }

    /// Write a message to the stream
    async fn write_message<W: AsyncWriteExt + Unpin>(&mut self, writer: &mut W, message: &Message) -> Result<()> {
        let serialized = message.serialize();
        writer.write_all(&serialized).await?;
        writer.flush().await?;
        Ok(())
    }

    /// Read a handshake from the stream
    async fn read_handshake<R: AsyncReadExt + Unpin>(&mut self, reader: &mut R) -> Result<Handshake> {
        // Read the protocol length (1 byte)
        let mut protocol_length_buf = [0u8; 1];
        reader.read_exact(&mut protocol_length_buf).await?;
        let protocol_length = protocol_length_buf[0] as usize;

        // Read the protocol string (protocol_length bytes)
        let mut protocol_buf = vec![0u8; protocol_length];
        reader.read_exact(&mut protocol_buf).await?;

        // Read extensions (1 byte) and reserved (7 bytes)
        let mut extensions_buf = [0u8; 8];
        reader.read_exact(&mut extensions_buf).await?;

        // Read info hash (20 bytes)
        let mut info_hash = [0u8; 20];
        reader.read_exact(&mut info_hash).await?;

        // Read peer ID (20 bytes)
        let mut peer_id = [0u8; 20];
        reader.read_exact(&mut peer_id).await?;

        // Construct the full handshake bytes
        let mut full_handshake = BytesMut::with_capacity(68);
        full_handshake.put_u8(protocol_length as u8);
        full_handshake.put_slice(&protocol_buf);
        full_handshake.put_slice(&extensions_buf);
        full_handshake.put_slice(&info_hash);
        full_handshake.put_slice(&peer_id);

        Handshake::deserialize(&full_handshake)
    }

    /// Write a handshake to the stream
    async fn write_handshake<W: AsyncWriteExt + Unpin>(&mut self, writer: &mut W, handshake: &Handshake) -> Result<()> {
        let serialized = handshake.serialize();
        writer.write_all(&serialized).await?;
        writer.flush().await?;
        Ok(())
    }
}

/// Read a length-prefixed message from the buffer
pub fn read_message(buf: &mut BytesMut) -> Result<Option<Vec<u8>>> {
    if buf.len() < 4 {
        return Ok(None);
    }

    let length = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;

    if buf.len() < 4 + length {
        return Ok(None);
    }

    let message = buf[4..4 + length].to_vec();
    buf.advance(4 + length);

    Ok(Some(message))
}

/// Write a length-prefixed message to the buffer
pub fn write_message(buf: &mut BytesMut, message: &[u8]) {
    buf.put_u32(message.len() as u32);
    buf.put_slice(message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_read_message() {
        let mut buf = BytesMut::new();
        let message = b"hello";
        write_message(&mut buf, message);

        let result = read_message(&mut buf).unwrap().unwrap();
        assert_eq!(result, message);
    }

    #[test]
    fn test_read_message_incomplete() {
        let mut buf = BytesMut::new();
        buf.put_u32(10); // Length prefix says 10 bytes
        buf.put_slice(b"hello"); // But only 5 bytes available

        let result = read_message(&mut buf).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_read_message_empty() {
        let mut buf = BytesMut::new();
        let result = read_message(&mut buf).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_write_read_empty_message() {
        let mut buf = BytesMut::new();
        let message = b"";
        write_message(&mut buf, message);

        let result = read_message(&mut buf).unwrap().unwrap();
        assert_eq!(result, message);
    }

    #[test]
    fn test_wire_protocol_trait_bounds() {
        // This test just verifies the trait is properly defined
        // It will compile if the trait signature is correct
        fn accepts_wire_protocol<T: WireProtocol>(_t: &mut T) {}
        let mut wire = BitTorrentWire;
        accepts_wire_protocol(&mut wire);
    }
}
