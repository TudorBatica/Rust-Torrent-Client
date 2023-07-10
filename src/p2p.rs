use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use crate::messages::Handshake;
use crate::tracker::Peer;

const PROTOCOL: &'static str = "BitTorrent protocol";

#[derive(Debug)]
pub enum P2PError {
    TCPConnectionUnestablished,
    HandshakeFailed,
    SocketClosed,
    IO(String),
}

pub async fn talk_to_peer(peer: Peer, info_hash: Vec<u8>, client_id: String) -> Result<(), P2PError> {
    let mut tcp_stream = establish_tcp_connection(&peer).await?;
    send_handshake(&mut tcp_stream, &peer, &info_hash, &client_id).await?;
    receive_handshake(&mut tcp_stream, &peer).await?;

    return Ok(());
}

async fn establish_tcp_connection(peer: &Peer) -> Result<TcpStream, P2PError> {
    println!("Establishing connection w/ peer {}:{}", peer.ip, peer.port);
    return match TcpStream::connect((peer.ip, peer.port)).await {
        Ok(stream) => {
            println!("TCP stream established w/ peer {}:{}", peer.ip, peer.port);
            Ok(stream)
        }
        Err(_) => {
            println!("Failed to establish TCP connection w/ peer {}:{}", peer.ip, peer.port);
            Err(P2PError::TCPConnectionUnestablished)
        }
    };
}

async fn send_handshake(stream: &mut TcpStream, peer: &Peer, info_hash: &Vec<u8>, client_id: &String) -> Result<(), P2PError> {
    let handshake = Handshake::new(&info_hash, &client_id);
    return match stream.write_all(&*handshake.content).await {
        Ok(_) => {
            println!("Handshake successfully sent to {}:{}", peer.ip, peer.port);
            Ok(())
        }
        Err(e) => {
            println!("Unable to send handshake to {}:{} -> {}", peer.ip, peer.port, e);
            Err(P2PError::HandshakeFailed)
        }
    };
}

async fn receive_handshake(stream: &mut TcpStream, peer: &Peer) -> Result<(), P2PError> {
    let pstrlen = match read_bytes(stream, 1).await {
        Ok(len) => len,
        Err(_) => {
            return Err(P2PError::HandshakeFailed);
        }
    };
    let _pstr = match read_bytes(stream, pstrlen[0] as usize).await {
        Ok(pstr) => pstr,
        Err(_) => {
            return Err(P2PError::HandshakeFailed);
        }
    };
    let _reserved = match read_bytes(stream, 8).await {
        Ok(res) => res,
        Err(_) => {
            return Err(P2PError::HandshakeFailed);
        }
    };
    let info_hash = match read_bytes(stream, 20).await {
        Ok(hash) => hash.iter().map(|byte| format!("{:02x}", byte)).collect::<String>(),
        Err(_) => {
            return Err(P2PError::HandshakeFailed);
        }
    };
    let peer_id = match read_bytes(stream, 20).await {
        Ok(id) => id.iter().map(|byte| format!("{:02x}", byte)).collect::<String>(),
        Err(_) => {
            return Err(P2PError::HandshakeFailed);
        }
    };
    println!("Received handshake from peer {}:{}\ninfo_hash:{}\npeer_id:{}", peer.ip, peer.port, info_hash, peer_id);

    return Ok(());
}

async fn read_bytes(stream: &mut TcpStream, mut num_of_bytes: usize) -> Result<Vec<u8>, P2PError> {
    let mut buffer: Vec<u8> = Vec::with_capacity(num_of_bytes);

    loop {
        let bytes_read = stream.take(num_of_bytes as u64).read_to_end(&mut buffer).await;
        match bytes_read {
            Ok(0) => {
                return Err(P2PError::SocketClosed);
            }
            Ok(n) => {
                if n == num_of_bytes {
                    return Ok(buffer);
                } else {
                    num_of_bytes -= n;
                }
            }
            Err(e) => {
                return Err(P2PError::IO(e.to_string()));
            }
        }
    }
}
