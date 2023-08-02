use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use crate::messages::Handshake;
use crate::tracker::Peer;

#[derive(Debug)]
pub enum P2PConnError {
    TCPConnectionFailed,
    HandshakeFailed,
    SocketClosed,
    IO(String),
}

//todo: add docs, this just handles talking to the peer, serializing/deserializing messages and establishes the tcp connection(+handshake)
pub struct PeerConnection {
    tcp_stream: TcpStream,
}

impl PeerConnection {
    pub async fn start_connection(peer: Peer, info_hash: Vec<u8>, client_id: String) -> Result<Self, P2PConnError> {
        let mut tcp_stream = Self::establish_tcp_connection(&peer).await?;
        Self::send_handshake(&mut tcp_stream, &peer, &info_hash, &client_id).await?;
        Self::receive_handshake(&mut tcp_stream, &peer).await?;

        println!("Connection with {}:{} started successfully!", peer.ip, peer.port);
        return Ok(PeerConnection { tcp_stream });
    }

    pub async fn accept_connection() {
        //todo: implement
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

    async fn send_handshake(stream: &mut TcpStream, peer: &Peer, info_hash: &Vec<u8>, client_id: &String) -> Result<(), P2PConnError> {
        let handshake = Handshake::new(&info_hash, &client_id);
        return match stream.write_all(&*handshake.content).await {
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
        let pstrlen = match Self::read_bytes(stream, 1).await {
            Ok(len) => len,
            Err(_) => {
                return Err(P2PConnError::HandshakeFailed);
            }
        };
        let _pstr = match Self::read_bytes(stream, pstrlen[0] as usize).await {
            Ok(pstr) => pstr,
            Err(_) => {
                return Err(P2PConnError::HandshakeFailed);
            }
        };
        let _reserved = match Self::read_bytes(stream, 8).await {
            Ok(res) => res,
            Err(_) => {
                return Err(P2PConnError::HandshakeFailed);
            }
        };
        let info_hash = match Self::read_bytes(stream, 20).await {
            Ok(hash) => hash.iter().map(|byte| format!("{:02x}", byte)).collect::<String>(),
            Err(_) => {
                return Err(P2PConnError::HandshakeFailed);
            }
        };
        let peer_id = match Self::read_bytes(stream, 20).await {
            Ok(id) => id.iter().map(|byte| format!("{:02x}", byte)).collect::<String>(),
            Err(_) => {
                return Err(P2PConnError::HandshakeFailed);
            }
        };
        println!("Received handshake from peer {}:{}\ninfo_hash:{}\npeer_id:{}", peer.ip, peer.port, info_hash, peer_id);

        return Ok(());
    }

    async fn read_bytes(stream: &mut TcpStream, mut num_of_bytes: usize) -> Result<Vec<u8>, P2PConnError> {
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
}

