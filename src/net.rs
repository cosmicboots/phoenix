#![allow(dead_code)]

use std::{
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
};

use snow::{Builder, TransportState};

static NOISE_PATTERN: &'static str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

pub trait NoiseConnection {
    fn new() -> Self;
    fn handshake(&mut self);
    fn send(&mut self, msg: &[u8]);
    fn recv(&mut self) -> Vec<u8>;
}

pub struct Server {
    stream: TcpStream,
    buf: Vec<u8>,
    noise: Option<TransportState>,
}

impl NoiseConnection for Server {
    fn new() -> Self {
        let (stream, _) = TcpListener::bind("127.0.0.1:8080")
            .expect("Failed to bind to server address")
            .accept()
            .unwrap();
        Server {
            stream,
            buf: vec![0u8; 65535],
            noise: None,
        }
    }

    /// Preform noise handshake
    fn handshake(&mut self) {
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
            .read_message(&recv(&mut self.stream).unwrap(), &mut self.buf)
            .unwrap();

        // -> e, ee, s, es
        // Send handhsake response
        let len = noise.write_message(&[0u8; 0], &mut self.buf).unwrap();
        send(&mut self.stream, &self.buf[..len]);

        // <- s, se
        // Finish handshake
        noise
            .read_message(&recv(&mut self.stream).unwrap(), &mut self.buf)
            .unwrap();

        // Finished handshake. Switch to transport mode
        self.noise = Some(noise.into_transport_mode().unwrap());
    }

    fn send(&mut self, msg: &[u8]) {
        if let Some(noise) = &mut self.noise {
            let len = noise.write_message(msg, &mut self.buf).unwrap();
            send(&mut self.stream, &self.buf[..len]);
        }
    }

    fn recv(&mut self) -> Vec<u8> {
        // TODO: improve error handling
        if let Some(noise) = &mut self.noise {
            match recv(&mut self.stream) {
                Ok(msg) => {
                    let len = noise.read_message(&msg, &mut self.buf).unwrap();
                    self.buf[..len].to_vec()
                }
                Err(_) => vec![],
            }
        } else {
            vec![]
        }
    }
}

pub struct Client {
    stream: TcpStream,
    buf: Vec<u8>,
    noise: Option<TransportState>,
}

impl NoiseConnection for Client {
    fn new() -> Self {
        Client {
            stream: TcpStream::connect("127.0.0.1:8080").unwrap(),
            buf: vec![0u8; 65535],
            noise: None,
        }
    }

    fn handshake(&mut self) {
        // Setup builder to start handshake
        let builder = Builder::new(NOISE_PATTERN.parse().unwrap());
        let static_key = builder.generate_keypair().unwrap().private;
        let mut noise = builder
            .local_private_key(&static_key)
            .build_initiator()
            .unwrap();

        // -> e
        // Initiate handshake
        let len = noise.write_message(&[], &mut self.buf).unwrap();
        send(&mut self.stream, &self.buf[..len]);

        // <- e, ee, s, es
        noise
            .read_message(&recv(&mut self.stream).unwrap(), &mut self.buf)
            .unwrap();

        // -> s, se
        let len = noise.write_message(&[], &mut self.buf).unwrap();
        send(&mut self.stream, &self.buf[..len]);

        self.noise = Some(noise.into_transport_mode().unwrap());
    }


    fn send(&mut self, msg: &[u8]) {
        if let Some(noise) = &mut self.noise {
            let len = noise.write_message(msg, &mut self.buf).unwrap();
            send(&mut self.stream, &self.buf[..len]);
        }
    }

    fn recv(&mut self) -> Vec<u8> {
        // TODO: improve error handling
        if let Some(noise) = &mut self.noise {
            match recv(&mut self.stream) {
                Ok(msg) => {
                    let len = noise.read_message(&msg, &mut self.buf).unwrap();
                    self.buf[..len].to_vec()
                }
                Err(_) => vec![],
            }
        } else {
            vec![]
        }
    }
}

fn recv(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut msg_len_buf = [0u8; 2];
    stream.read_exact(&mut msg_len_buf)?;
    //let msg_len = ((msg_len_buf[0] as usize) << 8) + (msg_len_buf[1] as usize);
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
