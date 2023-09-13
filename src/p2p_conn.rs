use async_trait::async_trait;
use mockall::automock;
use tokio::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use crate::core_models::entities::Peer;
use crate::core_models::entities::{Message};

const PROTOCOL: &'static str = "BitTorrent protocol";

#[derive(Debug)]
pub enum P2PConnError {
    TCPConnectionFailed,
    HandshakeFailed,
    SocketClosed,
    IO(String),
    UnknownMessageReceived,
    MessageDeliveryFailed(String),
}

#[async_trait]
pub trait PeerReceiver: Send {
    async fn receive(&mut self) -> Result<Message, P2PConnError>;
}

#[async_trait]
pub trait PeerSender: Send {
    async fn send(&mut self, message: Message) -> Result<(), P2PConnError>;
}

pub struct PeerReadConn {
    stream: ReadHalf<TcpStream>,
}

#[async_trait]
impl PeerReceiver for PeerReadConn {
    async fn receive(&mut self) -> Result<Message, P2PConnError> {
        let len = read_from_stream(&mut self.stream, 4).await?;
        let len = usize_from_be_bytes(len);
        if len == 0 {
            return Ok(Message::KeepAlive);
        }
        let message = read_from_stream(&mut self.stream, len).await?;
        return match Message::deserialize(message) {
            Some(msg) => Ok(msg),
            None => Err(P2PConnError::UnknownMessageReceived)
        };
    }
}

pub struct PeerWriteConn {
    stream: WriteHalf<TcpStream>,
}

#[async_trait]
impl PeerSender for PeerWriteConn {
    async fn send(&mut self, message: Message) -> Result<(), P2PConnError> {
        return match self.stream.write_all(&*message.serialize()).await {
            Ok(_) => Ok(()),
            Err(err) => Err(P2PConnError::MessageDeliveryFailed(err.to_string()))
        };
    }
}

#[async_trait]
#[automock]
pub trait PeerConnector: Send + Sync {
    async fn connect_to(&self, peer: Peer, info_hash: Vec<u8>, client_id: String) -> Result<(Box<dyn PeerReceiver>, Box<dyn PeerSender>), P2PConnError>;
}

pub struct TCPPeerConnector {}

#[async_trait]
impl PeerConnector for TCPPeerConnector {
    async fn connect_to(&self, peer: Peer, info_hash: Vec<u8>, client_id: String) -> Result<(Box<dyn PeerReceiver>, Box<dyn PeerSender>), P2PConnError> {
        let mut tcp_stream = establish_tcp_connection(&peer).await?;
        send_handshake(&mut tcp_stream, &peer, &info_hash, &client_id).await?;
        receive_handshake(&mut tcp_stream, &peer).await?;

        let (read_stream, write_stream) = io::split(tcp_stream);
        let receiver = Box::new(PeerReadConn { stream: read_stream });
        let sender = Box::new(PeerWriteConn { stream: write_stream });

        return Ok((receiver, sender));
    }
}

async fn establish_tcp_connection(peer: &Peer) -> Result<TcpStream, P2PConnError> {
    println!("Establishing connection w/ peer {}:{}", peer.ip, peer.port);
    return match TcpStream::connect((peer.ip, peer.port)).await {
        Ok(stream) => {
            println!("TCP stream established w/ peer {}:{}", peer.ip, peer.port);
            Ok(stream)
        }
        Err(_) => {
            println!("Failed to establish TCP connection w/ peer {}:{}", peer.ip, peer.port);
            Err(P2PConnError::TCPConnectionFailed)
        }
    };
}

async fn read_from_stream<T: AsyncRead + Unpin>(stream: &mut T, mut num_of_bytes: usize) -> Result<Vec<u8>, P2PConnError> {
    let mut buffer: Vec<u8> = Vec::with_capacity(num_of_bytes);

    loop {
        let bytes_read = stream.take(num_of_bytes as u64).read_to_end(&mut buffer).await;
        match bytes_read {
            Ok(0) => {
                return Err(P2PConnError::SocketClosed);
            }
            Ok(n) => {
                if n == num_of_bytes {
                    return Ok(buffer);
                } else {
                    num_of_bytes -= n;
                }
            }
            Err(e) => {
                return Err(P2PConnError::IO(e.to_string()));
            }
        }
    }
}

async fn send_handshake(stream: &mut TcpStream, peer: &Peer, info_hash: &Vec<u8>, client_id: &String) -> Result<(), P2PConnError> {
    let mut handshake: Vec<u8> = Vec::with_capacity(49 + PROTOCOL.len());
    //pstrlen
    handshake.push(PROTOCOL.len() as u8);
    //pstr
    handshake.extend(PROTOCOL.bytes());
    //reserved bytes
    handshake.extend(vec![0; 8].into_iter());
    //info hash of desired torrent
    handshake.extend(info_hash);
    //client id
    handshake.extend(client_id.bytes());

    return match stream.write_all(&*handshake).await {
        Ok(_) => {
            println!("Handshake successfully sent to {}:{}", peer.ip, peer.port);
            Ok(())
        }
        Err(e) => {
            println!("Unable to send handshake to {}:{} -> {}", peer.ip, peer.port, e);
            Err(P2PConnError::HandshakeFailed)
        }
    };
}

async fn receive_handshake(stream: &mut TcpStream, peer: &Peer) -> Result<(), P2PConnError> {
    //todo: check all props in handshake!
    let pstrlen = match read_from_stream(stream, 1).await {
        Ok(len) => len,
        Err(_) => {
            return Err(P2PConnError::HandshakeFailed);
        }
    };
    let _pstr = match read_from_stream(stream, pstrlen[0] as usize).await {
        Ok(pstr) => pstr,
        Err(_) => {
            return Err(P2PConnError::HandshakeFailed);
        }
    };
    let _reserved = match read_from_stream(stream, 8).await {
        Ok(res) => res,
        Err(_) => {
            return Err(P2PConnError::HandshakeFailed);
        }
    };
    let info_hash = match read_from_stream(stream, 20).await {
        Ok(hash) => hash.iter().map(|byte| format!("{:02x}", byte)).collect::<String>(),
        Err(_) => {
            return Err(P2PConnError::HandshakeFailed);
        }
    };
    let peer_id = match read_from_stream(stream, 20).await {
        Ok(id) => id.iter().map(|byte| format!("{:02x}", byte)).collect::<String>(),
        Err(_) => {
            return Err(P2PConnError::HandshakeFailed);
        }
    };
    println!("Received handshake from peer {}:{}\ninfo_hash:{}\npeer_id:{}", peer.ip, peer.port, info_hash, peer_id);

    return Ok(());
}

fn usize_from_be_bytes(bytes: Vec<u8>) -> usize {
    return bytes.into_iter()
        .rev().enumerate()
        .map(|(idx, byte)| 256usize.pow(idx as u32) * byte as usize)
        .sum();
}
