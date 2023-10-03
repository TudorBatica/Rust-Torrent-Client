use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;
use tokio::time;
use crate::{p2p};
use crate::core_models::entities::{Bitfield, Peer};
use crate::dependency_provider::TransferDeps;
use crate::p2p::conn::PeerReceiver;
use crate::p2p::state::{FunnelMsg, P2PInboundEvent, P2PState, P2PTransferError};

pub async fn spawn(peer: Peer,
                   transfer_idx: usize,
                   client_bitfield: Bitfield,
                   deps: Arc<dyn TransferDeps>,
) -> (JoinHandle<Result<(), P2PTransferError>>, Sender<P2PInboundEvent>) {
    let (tx_to_self, rx) = mpsc::channel::<P2PInboundEvent>(2048);
    let state = P2PState::new(transfer_idx, client_bitfield, deps.torrent_layout().pieces);

    let handle = tokio::spawn(async move {
        return run(peer, deps, state, rx).await;
    });

    return (handle, tx_to_self);
}

async fn run(peer: Peer,
             deps: Arc<dyn TransferDeps>,
             mut state: P2PState,
             rx: Receiver<P2PInboundEvent>) -> Result<(), P2PTransferError> {
    // connect to peer
    let client_id = deps.client_config().client_id;
    let info_hash = deps.info_hash();
    let connector = deps.peer_connector();
    let (read_conn, mut write_conn) = connector.connect_to(peer, info_hash, client_id).await.unwrap();

    // initialize needed dependencies
    let output_tx = deps.output_tx();
    let picker = deps.piece_picker();
    let mut file_provider = deps.file_provider();
    file_provider.open_read_only_instance().await;

    // start p2p and internal events listeners and merge them into one
    let (funnel_tx, mut funnel_rx) = mpsc::channel::<FunnelMsg>(128);
    let peer_handle = tokio::spawn(recv_peer_messages(read_conn, funnel_tx.clone()));
    let events_handle = tokio::spawn(recv_internal_events(rx, funnel_tx.clone()));
    let keepalive_handle = tokio::spawn(recv_keep_alive_scheduler_events(funnel_tx.clone()));

    while let Some(data) = funnel_rx.recv().await {
        //todo: refactor peer-conn failed message
        match data.clone() {
            FunnelMsg::InternalEvent(event) => {
                match event {
                    P2PInboundEvent::PeerConnFailed => {
                        println!("Error in p2p task {}", state.transfer_idx);
                        events_handle.abort();
                        keepalive_handle.abort();
                        return Err(P2PTransferError::PeerConnFailed);
                    }
                    _ => {}
                }
            }
            FunnelMsg::PeerMessage(_) => {}
        };
        let handler_result = p2p::handlers::handle(
            data, &mut state, &mut file_provider, &picker,
        ).await;
        for message in handler_result.messages_for_peer {
            println!("{} sending message {:?}", state.transfer_idx, message);
            write_conn.send(message).await.unwrap();
        }
        for event in handler_result.internal_events {
            println!("{} sending event", state.transfer_idx);
            output_tx.send(event).await.unwrap();
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
                println!("Encountered p2p error :: {:?}", err);
                tx.send(FunnelMsg::InternalEvent(P2PInboundEvent::PeerConnFailed)).await.unwrap();
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