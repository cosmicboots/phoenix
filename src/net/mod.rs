//! Network module. Main purpose is to wrap traffic in the noise protocol.
//!
//! This is only a lightweight wrapper for handling noise protocol connections using Snow.
//! This module needs to be used in conjunction with [`messaging`](../messaging/index.html) to
//! successfully send messages.
//!
//! The two main structs are the [`client`](struct.Client.html) and [`server`](struct.Server.html),
//! which both implement the [`NoiseConnection`](trait.NoiseConnection.html) trait.
//!
//! ## Server Example
//!
//! ```rust
//! // Construct TcpListener
//! let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
//! // Iterate through connections
//! for stream in listener.incoming() {
//!     let mut server = Server::new(
//!         stream.unwrap(),
//!         &noise_private_key,
//!         &valid_client_pubkeys,
//!     ).unwrap()
//!     
//!     // Receive message
//!     if let Ok(msg) = &server.recv() {
//!         println!("{:?}", msg);
//!     }
//! }
//! ```
//! **Note:** _The above example only allows a single TCP connection at a time._
//!
//! ## Client example
//!
//! ```rust
//! // Create client to wrap messages with the noise protocol
//! let mut client = Client::new(
//!     TcpStream::connect("127.0.0.1:8080").unwrap(),
//!     &noise_private_key,
//!     &[server_public_key],
//! )
//! .unwrap();
//!
//! // Create MessageBuilder to create messages to send
//! let mut builder = messaging::MessageBuilder::new(1);
//! // Create a message
//! let msg = builder.encode_message(Directive::AnnounceVersion, Some(arguments::Version(1)));
//! // Send the message
//! client.send(&msg).unwrap();
//! ```

pub mod error;

use async_trait::async_trait;
use base64ct::{Base64, Encoding};
use error::NetError;
use log::{debug, error};
use snow::{Builder, Keypair, TransportState};
use tokio::{io::AsyncReadExt, io::AsyncWriteExt, net::TcpStream};

static NOISE_PATTERN: &str = "Noise_IK_25519_ChaChaPoly_BLAKE2s";

#[async_trait]
/// A generic trait that allows noise connections to be created and send/recieve information
pub trait NoiseConnection {
    async fn new(
        stream: TcpStream,
        static_key: &[u8],
        remote_keys: &[Vec<u8>],
    ) -> Result<Self, NetError>
    where
        Self: Sized;
    async fn send(&mut self, msg: &[u8]) -> Result<(), NetError>;
    async fn recv(&mut self) -> Result<Vec<u8>, NetError>;
}

/// The server side of the network connection
///
/// `NetServer` will be the responder in the Noise handshake while the
/// [`NetClient`](struct.NetClient.html) will be the initiator.
pub struct NetServer {
    stream: TcpStream,
    buf: Vec<u8>,
    noise: TransportState,
}

#[async_trait]
impl NoiseConnection for NetServer {
    async fn new(
        mut stream: TcpStream,
        static_key: &[u8],
        remote_keys: &[Vec<u8>],
    ) -> Result<Self, NetError> {
        let mut buf = vec![0u8; 65535];

        // Setup builder to start handshake
        let builder = Builder::new(NOISE_PATTERN.parse().unwrap());
        let mut noise = builder.local_private_key(static_key).build_responder()?;

        // <- e, es, s, ss
        noise.read_message(&recv(&mut stream).await?, &mut buf)?;

        // At this point, we have the initiator's static key and we can check if it's in our
        // allowed list of keys
        debug!(
            "Initiator's public key: {}",
            Base64::encode_string(noise.get_remote_static().unwrap())
        );

        let is = noise.get_remote_static().unwrap();
        if !remote_keys.contains(&is.to_vec()) {
            error!("Remote public key isn't known");
        }

        // -> e, ee, se
        let len = noise.write_message(&[0u8; 0], &mut buf)?;
        send(&mut stream, &buf[..len]).await?;

        // Finished handshake. Switch to transport mode
        let noise = noise.into_transport_mode()?;
        Ok(NetServer { stream, buf, noise })
    }

    async fn send(&mut self, msg: &[u8]) -> Result<(), NetError> {
        let len = self.noise.write_message(msg, &mut self.buf)?;
        send(&mut self.stream, &self.buf[..len]).await?;
        Ok(())
    }

    async fn recv(&mut self) -> Result<Vec<u8>, NetError> {
        let len = self
            .noise
            .read_message(&recv(&mut self.stream).await?, &mut self.buf)?;
        Ok(self.buf[..len].to_vec())
    }
}

/// The client side of the network connection
///
/// `NetClient` will be the initiator in the Noise handshake while the
/// [`NetServer`](struct.NetServer.html) will be the responder.
pub struct NetClient {
    stream: TcpStream,
    buf: Vec<u8>,
    noise: TransportState,
}

#[async_trait]
impl NoiseConnection for NetClient {
    async fn new(
        mut stream: TcpStream,
        static_key: &[u8],
        remote_keys: &[Vec<u8>],
    ) -> Result<Self, NetError> {
        let mut buf = vec![0u8; 65535];

        // Setup builder to start handshake
        let builder = Builder::new(NOISE_PATTERN.parse()?);

        let mut noise = builder
            .local_private_key(static_key)
            .remote_public_key(&remote_keys[0])
            .build_initiator()
            .unwrap();

        // -> e, es, s, ss
        let len = noise.write_message(&[], &mut buf)?;
        send(&mut stream, &buf[..len]).await?;

        // <- e, ee, se
        noise.read_message(&recv(&mut stream).await?, &mut buf)?;

        let noise = noise.into_transport_mode()?;
        Ok(NetClient { stream, buf, noise })
    }

    async fn send(&mut self, msg: &[u8]) -> Result<(), NetError> {
        if msg.len() > 65535 {
            return Err(NetError::MsgLength(msg.len()));
        }
        let len = self.noise.write_message(msg, &mut self.buf)?;
        send(&mut self.stream, &self.buf[..len]).await?;
        Ok(())
    }

    async fn recv(&mut self) -> Result<Vec<u8>, NetError> {
        let len = self
            .noise
            .read_message(&recv(&mut self.stream).await?, &mut self.buf)?;
        Ok(self.buf[..len].to_vec())
    }
}

pub async fn recv(stream: &mut TcpStream) -> Result<Vec<u8>, NetError> {
    let mut msg_len_buf = [0u8; 2];
    stream.read_exact(&mut msg_len_buf).await?;
    let msg_len = u16::from_be_bytes(msg_len_buf) as usize;
    let mut msg = vec![0u8; msg_len];
    stream.read_exact(&mut msg[..]).await?;
    Ok(msg)
}

async fn send(stream: &mut TcpStream, msg: &[u8]) -> Result<(), NetError> {
    let msg_len = (msg.len() as u16).to_be_bytes();
    // Time out might be needed here...?
    stream.write_all(&msg_len).await?;
    stream.write_all(msg).await?;
    Ok(())
}

/// Generate a noise key pair
///
/// This creates a [Noise Protocol](https://noiseprotocol.org) keypair using the
/// `Noise_IK_25519_ChaChaPoly_BLAKE2s` noise pattern.
///
/// The keypair will be used to preform the client-server network handshake and should be included
/// in the [config](config/index.html).
pub fn generate_noise_keypair() -> Keypair {
    let builder = Builder::new(NOISE_PATTERN.parse().unwrap());
    builder.generate_keypair().unwrap()
}
