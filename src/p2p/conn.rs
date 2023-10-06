use tokio::net::{TcpStream};
use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use mockall::automock;
use tokio::io;
use crate::core_models::entities::{Message, Peer};
use crate::p2p::models::P2PError;

const PROTOCOL: &'static str = "BitTorrent protocol";

#[async_trait]
pub trait PeerReceiver: Send {
    async fn receive(&mut self) -> Result<Message, P2PError>;
}

#[async_trait]
pub trait PeerSender: Send {
    async fn send(&mut self, message: Message) -> Result<(), P2PError>;
}

pub struct PeerReadConn {
    stream: ReadHalf<TcpStream>,
}

#[async_trait]
impl PeerReceiver for PeerReadConn {
    async fn receive(&mut self) -> Result<Message, P2PError> {
        let len = read_from_stream(&mut self.stream, 4).await?;
        let len = usize_from_be_bytes(len);
        let message = read_from_stream(&mut self.stream, len).await?;
        return match Message::deserialize(message) {
            Some(msg) => Ok(msg),
            None => Err(P2PError::UnknownMessageReceived)
        };
    }
}

pub struct PeerWriteConn {
    stream: WriteHalf<TcpStream>,
}

#[async_trait]
impl PeerSender for PeerWriteConn {
    async fn send(&mut self, message: Message) -> Result<(), P2PError> {
        return match self.stream.write_all(&*message.serialize()).await {
            Ok(_) => Ok(()),
            Err(err) => Err(P2PError::MessageDeliveryFailed(err.to_string()))
        };
    }
}

#[async_trait]
#[automock]
pub trait PeerConnector: Send + Sync {
    async fn connect_to(&self, peer: Peer, info_hash: Vec<u8>, client_id: String) -> Result<(Box<dyn PeerReceiver>, Box<dyn PeerSender>), P2PError>;
}

pub struct TCPPeerConnector {}

#[async_trait]
impl PeerConnector for TCPPeerConnector {
    async fn connect_to(&self, peer: Peer, info_hash: Vec<u8>, client_id: String) -> Result<(Box<dyn PeerReceiver>, Box<dyn PeerSender>), P2PError> {
        let mut tcp_stream = establish_tcp_connection(&peer).await?;
        send_handshake(&mut tcp_stream, &peer, &info_hash, &client_id).await?;
        receive_handshake(&mut tcp_stream).await?;

        let (read_stream, write_stream) = io::split(tcp_stream);
        let receiver = Box::new(PeerReadConn { stream: read_stream });
        let sender = Box::new(PeerWriteConn { stream: write_stream });

        return Ok((receiver, sender));
    }
}

async fn establish_tcp_connection(peer: &Peer) -> Result<TcpStream, P2PError> {
    return match TcpStream::connect((peer.ip, peer.port)).await {
        Ok(stream) => Ok(stream),
        Err(_) => Err(P2PError::TCPConnectionNotEstablished),
    };
}

async fn read_from_stream<T: AsyncRead + Unpin>(stream: &mut T, mut num_of_bytes: usize) -> Result<Vec<u8>, P2PError> {
    if num_of_bytes == 0 {
        return Ok(vec![]);
    }

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

async fn send_handshake(stream: &mut TcpStream, peer: &Peer, info_hash: &Vec<u8>, client_id: &String) -> Result<(), P2PError> {
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
        Ok(_) => Ok(()),
        Err(_) => Err(P2PError::HandshakeFailed)
    };
}

async fn receive_handshake(stream: &mut TcpStream) -> Result<(), P2PError> {
    //todo: check all props in handshake!
    let pstrlen = match read_from_stream(stream, 1).await {
        Ok(len) => len,
        Err(_) => {
            return Err(P2PError::HandshakeFailed);
        }
    };
    let _pstr = match read_from_stream(stream, pstrlen[0] as usize).await {
        Ok(pstr) => pstr,
        Err(_) => {
            return Err(P2PError::HandshakeFailed);
        }
    };
    let _reserved = match read_from_stream(stream, 8).await {
        Ok(res) => res,
        Err(_) => {
            return Err(P2PError::HandshakeFailed);
        }
    };
    let _info_hash = match read_from_stream(stream, 20).await {
        Ok(hash) => hash.iter().map(|byte| format!("{:02x}", byte)).collect::<String>(),
        Err(_) => {
            return Err(P2PError::HandshakeFailed);
        }
    };
    let _peer_id = match read_from_stream(stream, 20).await {
        Ok(id) => id.iter().map(|byte| format!("{:02x}", byte)).collect::<String>(),
        Err(_) => {
            return Err(P2PError::HandshakeFailed);
        }
    };
    return Ok(());
}

fn usize_from_be_bytes(bytes: Vec<u8>) -> usize {
    return bytes.into_iter()
        .rev().enumerate()
        .map(|(idx, byte)| 256usize.pow(idx as u32) * byte as usize)
        .sum();
}

#[cfg(test)]
mod tests {
    use tokio::io::AsyncWriteExt;
    use tokio::net::{TcpListener, TcpStream};
    use crate::core_models::entities::{DataBlock, Message};
    use crate::p2p::conn::{PeerReadConn, PeerReceiver};

    #[tokio::test]
    async fn test_receive_message() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local_addr = listener.local_addr().unwrap();

        let task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (read_half, _) = tokio::io::split(stream);
            let mut receiver = PeerReadConn { stream: read_half };
            return receiver.receive().await.unwrap();
        });

        let mut client_stream = TcpStream::connect(&local_addr).await.unwrap();
        let piece_msg = vec![0, 0, 0, 10, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0]; // a piece message for piece idx 0, begin = 0, data = [0]
        client_stream.write_all(&piece_msg).await.unwrap();

        let received_message = task.await.unwrap();
        assert_eq!(received_message, Message::Piece(DataBlock::new(0, 0, vec![0])));
    }
}

