use std::sync::Arc;
use std::time::Duration;
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
use crate::p2p::state::{FunnelMsg, P2PInboundEvent, P2PState, P2PError};

pub async fn spawn(peer: Peer,
                   transfer_idx: usize,
                   client_bitfield: Bitfield,
                   deps: Arc<dyn TransferDeps>,
) -> (JoinHandle<Result<(), P2PError>>, Sender<P2PInboundEvent>) {
    let (tx_to_self, rx) = mpsc::channel::<P2PInboundEvent>(8192);
    let state = P2PState::new(transfer_idx, client_bitfield, deps.torrent_layout().pieces);

    let handle = tokio::spawn(async move {
        return run(peer, deps, state, rx).await;
    });

    return (handle, tx_to_self);
}

async fn run(peer: Peer,
             deps: Arc<dyn TransferDeps>,
             mut state: P2PState,
             rx: Receiver<P2PInboundEvent>) -> Result<(), P2PError> {
    let output_tx = deps.output_tx();
    let picker = deps.piece_picker();
    let mut file_provider = deps.file_provider();

    let connection = timeout(
        Duration::from_secs(10),
        deps.peer_connector().connect_to(peer, deps.info_hash(), deps.client_config().client_id),
    ).await;
    let connection = match connection {
        Ok(conn_result) => { conn_result }
        Err(err) => {
            println!("P2P Transfer {} terminated due to {:?}", state.transfer_idx, err);
            output_tx.send(InternalEvent::P2PTransferTerminated(state.transfer_idx)).await.unwrap();
            return Err(P2PError::TCPConnectionNotEstablished);
        }
    };
    let (read_conn, mut write_conn) = match connection {
        Ok((read, write)) => { (read, write) }
        Err(err) => {
            println!("P2P Transfer {} terminated due to {:?}", state.transfer_idx, err);
            output_tx.send(InternalEvent::P2PTransferTerminated(state.transfer_idx)).await.unwrap();
            return Err(err);
        }
    };

    output_tx.send(InternalEvent::PeerConnectionEstablished(state.transfer_idx)).await.unwrap();
    file_provider.open_read_only_instance().await;

    // start p2p and internal events listeners and merge them into one
    let (funnel_tx, mut funnel_rx) = mpsc::channel::<FunnelMsg>(128);
    let peer_handle = tokio::spawn(recv_peer_messages(read_conn, funnel_tx.clone()));
    let events_handle = tokio::spawn(recv_internal_events(rx, funnel_tx.clone()));
    let keepalive_handle = tokio::spawn(recv_keep_alive_scheduler_events(funnel_tx.clone()));

    while let Some(data) = funnel_rx.recv().await {
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
                println!("P2P Transfer {} terminated due to {:?}", state.transfer_idx, err);
                output_tx.send(InternalEvent::P2PTransferTerminated(state.transfer_idx)).await.unwrap();
                events_handle.abort();
                keepalive_handle.abort();
                return Err(err);
            }
        }
    }

    peer_handle.abort();
    events_handle.abort();
    keepalive_handle.abort();

    return Ok(());
}

async fn recv_peer_messages(mut conn: Box<dyn PeerReceiver>, tx: Sender<FunnelMsg>) {
    loop {
        let message = conn.receive().await;
        let _ = match message {
            Ok(msg) => tx.send(FunnelMsg::PeerMessage(msg)).await.unwrap(),
            Err(err) => {
                tx.send(FunnelMsg::P2PFailure(err)).await.unwrap();
                break;
            }
        };
    }
}

async fn recv_internal_events(mut events_rx: Receiver<P2PInboundEvent>, tx: Sender<FunnelMsg>) {
    while let Some(event) = events_rx.recv().await {
        tx.send(FunnelMsg::InternalEvent(event)).await.unwrap();
    }
}

async fn recv_keep_alive_scheduler_events(tx: Sender<FunnelMsg>) {
    let mut interval = time::interval(Duration::from_secs(50));
    loop {
        interval.tick().await;
        tx.send(FunnelMsg::InternalEvent(P2PInboundEvent::SendKeepAlive)).await.unwrap();
    }
}