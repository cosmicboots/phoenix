#![allow(dead_code)]

use std::{
    io::{self, Read, Write},
    net::TcpStream,
};

use base64ct::{Base64, Encoding};
use snow::{Builder, Keypair, TransportState};

static NOISE_PATTERN: &'static str = "Noise_IK_25519_ChaChaPoly_BLAKE2s";

pub trait NoiseConnection {
    fn new(stream: TcpStream, static_key: &[u8], remote_key: &[u8]) -> Self;
    fn send(&mut self, msg: &[u8]);
    fn recv(&mut self) -> io::Result<Vec<u8>>;
}

pub struct Server {
    stream: TcpStream,
    buf: Vec<u8>,
    noise: TransportState,
}

impl NoiseConnection for Server {
    fn new(mut stream: TcpStream, static_key: &[u8], _: &[u8]) -> Self {
        let mut buf = vec![0u8; 65535];

        // Setup builder to start handshake
        let builder = Builder::new(NOISE_PATTERN.parse().unwrap());
        let mut noise = builder
            .local_private_key(static_key)
            .build_responder()
            .unwrap();

        // <- e, es, s, ss
        noise
            .read_message(&recv(&mut stream).unwrap(), &mut buf)
            .unwrap();

        // At this point, we have the initiator's static key and we can check if it's in our
        // allowed list of keys
        debug!("Initiator's public key: {:?}", Base64::encode_string(noise.get_remote_static().unwrap()));

        // -> e, ee, se
        let len = noise.write_message(&[0u8; 0], &mut buf).unwrap();
        send(&mut stream, &buf[..len]);

        // Finished handshake. Switch to transport mode
        let noise = noise.into_transport_mode().unwrap();
        Server { stream, buf, noise }
    }

    fn send(&mut self, msg: &[u8]) {
        let len = self.noise.write_message(msg, &mut self.buf).unwrap();
        send(&mut self.stream, &self.buf[..len]);
    }

    fn recv(&mut self) -> io::Result<Vec<u8>> {
        let len = self
            .noise
            .read_message(&recv(&mut self.stream)?, &mut self.buf)
            .unwrap();
        Ok(self.buf[..len].to_vec())
    }
}

pub struct Client {
    stream: TcpStream,
    buf: Vec<u8>,
    noise: TransportState,
}

impl NoiseConnection for Client {
    fn new(mut stream: TcpStream, static_key: &[u8], remote_key: &[u8]) -> Self {
        let mut buf = vec![0u8; 65535];

        // Setup builder to start handshake
        let builder = Builder::new(NOISE_PATTERN.parse().unwrap());

        let mut noise = builder
            .local_private_key(static_key)
            .remote_public_key(remote_key)
            .build_initiator()
            .unwrap();

        // -> e, es, s, ss
        let len = noise.write_message(&[], &mut buf).unwrap();
        send(&mut stream, &buf[..len]);

        // <- e, ee, se
        noise
            .read_message(&recv(&mut stream).unwrap(), &mut buf)
            .unwrap();

        debug!(
            "Handshake finished....?: {}",
            <base64ct::Base64 as base64ct::Encoding>::encode_string(
                noise.get_remote_static().unwrap()
            )
        );

        let noise = noise.into_transport_mode().unwrap();
        Client { stream, buf, noise }
    }

    fn send(&mut self, msg: &[u8]) {
        let len = self.noise.write_message(msg, &mut self.buf).unwrap();
        send(&mut self.stream, &self.buf[..len]);
    }

    fn recv(&mut self) -> io::Result<Vec<u8>> {
        let len = self
            .noise
            .read_message(&recv(&mut self.stream)?, &mut self.buf)
            .unwrap();
        Ok(self.buf[..len].to_vec())
    }
}

fn recv(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut msg_len_buf = [0u8; 2];
    stream.read_exact(&mut msg_len_buf)?;
    let msg_len = u16::from_be_bytes(msg_len_buf) as usize;
    let mut msg = vec![0u8; msg_len];
    stream.read_exact(&mut msg[..])?;
    Ok(msg)
}

fn send(stream: &mut TcpStream, msg: &[u8]) {
    let msg_len = (msg.len() as u16).to_be_bytes();
    // Time out might be needed here...?
    stream.write_all(&msg_len).unwrap();
    stream.write_all(msg).unwrap();
}

pub fn generate_noise_keypair() -> Keypair {
    let builder = Builder::new(NOISE_PATTERN.parse().unwrap());
    builder.generate_keypair().unwrap()
}
