use std::sync::Arc;
use std::time::Duration;
use log::warn;
use tokio::sync::{mpsc};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;
use tokio::time;
use tokio::time::timeout;
use crate::{p2p};
use crate::core_models::entities::{Bitfield, Peer};
use crate::core_models::events::InternalEvent;
use crate::dependency_provider::TransferDeps;
use crate::p2p::conn::{PeerReceiver, PeerSender};
use crate::p2p::models::{P2PEvent, P2PState, P2PError};

pub fn spawn(peer: Peer,
                   transfer_idx: usize,
                   client_bitfield: Bitfield,
                   deps: Arc<dyn TransferDeps>,
) -> (JoinHandle<Result<(), P2PError>>, Sender<P2PEvent>) {
    let (tx_to_self, rx) = mpsc::channel::<P2PEvent>(8192);
    let state = P2PState::new(transfer_idx, client_bitfield, deps.torrent_layout().pieces);
    let tx_to_self_clone = tx_to_self.clone();
    let handle = tokio::spawn(async move {
        return run(peer, deps, state, rx, tx_to_self_clone).await;
    });

    return (handle, tx_to_self);
}

async fn run(peer: Peer,
             deps: Arc<dyn TransferDeps>,
             mut state: P2PState,
             mut rx: Receiver<P2PEvent>,
             tx_to_self: Sender<P2PEvent>,
) -> Result<(), P2PError> {
    let output_tx = deps.output_tx();
    let picker = deps.piece_picker();
    let mut file_provider = deps.file_provider();

    let (read_conn, mut write_conn) = match connect_to_peer(&deps, peer).await {
        Ok((read, write)) => { (read, write) }
        Err(err) => {
            warn!("P2P Transfer {} terminated due to {:?}", state.transfer_idx, err);
            output_tx.send(InternalEvent::P2PTransferTerminated(state.transfer_idx)).await.unwrap();
            return Err(err);
        }
    };

    output_tx.send(InternalEvent::PeerConnectionEstablished(state.transfer_idx)).await.unwrap();
    file_provider.open_read_only_instance().await;

    let peer_msg_handler = tokio::spawn(recv_peer_messages(read_conn, tx_to_self.clone()));
    let keep_alive_handler = tokio::spawn(keep_alive_event_scheduler(tx_to_self));

    while let Some(data) = rx.recv().await {
        let handler_result = p2p::handlers::handle(
            data, &mut state, &mut file_provider, &picker,
        ).await;
        match handler_result {
            Ok(result) => {
                for message in result.messages_for_peer {
                    write_conn.send(message).await.unwrap();
                }
                for event in result.internal_events {
                    output_tx.send(event).await.unwrap();
                }
            }
            Err(err) => {
                warn!("P2P Transfer {} terminated due to {:?}", state.transfer_idx, err);
                output_tx.send(InternalEvent::P2PTransferTerminated(state.transfer_idx)).await.unwrap();
                keep_alive_handler.abort();
                return Err(err);
            }
        }
    }

    peer_msg_handler.abort();
    keep_alive_handler.abort();

    return Ok(());
}

async fn recv_peer_messages(mut conn: Box<dyn PeerReceiver>, tx: Sender<P2PEvent>) {
    loop {
        let message = conn.receive().await;
        let is_err = message.is_err();
        tx.send(P2PEvent::PeerMessageReceived(message)).await.unwrap();
        if is_err {
            break;
        }
    }
}

async fn keep_alive_event_scheduler(tx: Sender<P2PEvent>) {
    let mut interval = time::interval(Duration::from_secs(50));
    loop {
        interval.tick().await;
        tx.send(P2PEvent::SendKeepAlive).await.unwrap();
    }
}

async fn connect_to_peer(deps: &Arc<dyn TransferDeps>, peer: Peer)
                         -> Result<(Box<dyn PeerReceiver>, Box<dyn PeerSender>), P2PError> {
    let connection = timeout(
        Duration::from_secs(10),
        deps.peer_connector().connect_to(peer, deps.info_hash(), deps.client_config().client_id),
    ).await;

    return match connection {
        Ok(conn_result) => conn_result,
        Err(_) => return Err(P2PError::TCPConnectionNotEstablished),
    };
}