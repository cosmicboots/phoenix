#![allow(dead_code)]

use std::{
    io::{self, Read, Write},
    net::TcpStream,
};

use snow::{Builder, TransportState};

static NOISE_PATTERN: &'static str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

pub trait NoiseConnection {
    fn new(stream: TcpStream) -> Self;
    //fn handshake(&mut self);
    fn send(&mut self, msg: &[u8]);
    fn recv(&mut self) -> io::Result<Vec<u8>>;
}

pub struct Server {
    stream: TcpStream,
    buf: Vec<u8>,
    noise: TransportState,
}

impl NoiseConnection for Server {
    fn new(mut stream: TcpStream) -> Self {
        let mut buf = vec![0u8; 65535];

        // Setup builder to start handshake
        let builder = Builder::new(NOISE_PATTERN.parse().unwrap());
        let static_key = builder.generate_keypair().unwrap().private;
        let mut noise = builder
            .local_private_key(&static_key)
            .build_responder()
            .unwrap();

        // <- e
        // Start handshake
        noise
            .read_message(&recv(&mut stream).unwrap(), &mut buf)
            .unwrap();

        // -> e, ee, s, es
        // Send handhsake response
        let len = noise.write_message(&[0u8; 0], &mut buf).unwrap();
        send(&mut stream, &buf[..len]);

        // <- s, se
        // Finish handshake
        noise
            .read_message(&recv(&mut stream).unwrap(), &mut buf)
            .unwrap();

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
    fn new(mut stream: TcpStream) -> Self {
        let mut buf = vec![0u8; 65535];

        // Setup builder to start handshake
        let builder = Builder::new(NOISE_PATTERN.parse().unwrap());
        let static_key = builder.generate_keypair().unwrap().private;
        let mut noise = builder
            .local_private_key(&static_key)
            .build_initiator()
            .unwrap();

        // -> e
        // Initiate handshake
        let len = noise.write_message(&[], &mut buf).unwrap();
        send(&mut stream, &buf[..len]);

        // <- e, ee, s, es
        noise
            .read_message(&recv(&mut stream).unwrap(), &mut buf)
            .unwrap();

        // -> s, se
        let len = noise.write_message(&[], &mut buf).unwrap();
        send(&mut stream, &buf[..len]);

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
