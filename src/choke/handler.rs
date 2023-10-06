use std::collections::HashMap;
use rand::prelude::IteratorRandom;
use crate::choke::state::{ChokeEvent, PeerState};
use crate::core_models::events::InternalEvent;

const MAX_CONCURRENTLY_UNCHOKED_PEERS: usize = 4;

pub fn handle(event: ChokeEvent, peers: &mut HashMap<usize, PeerState>) -> Vec<InternalEvent> {
    return match event {
        ChokeEvent::UnchokePeers => {
            unchoke_peers(peers)
        }
        ChokeEvent::OptimisticUnchoke => {
            optimistic_unchoke(peers)
        }
        ChokeEvent::ClientInterestedInPeer(idx, interested) => {
            peers.get_mut(&idx).unwrap().client_interested_in_peer = interested;
            vec![]
        }
        ChokeEvent::PeerInterestedInClient(idx, interested) => {
            println!("peer {} interested in client", idx);
            peers.get_mut(&idx).unwrap().peer_interested_in_client = interested;
            vec![]
        }
        ChokeEvent::BlockDownloadedFromPeer(idx) => {
            peers.get_mut(&idx).unwrap().block_downloaded();
            vec![]
        }
        ChokeEvent::UnregisterPeer(idx) => {
            peers.remove(&idx);
            vec![]
        }
    };
}

fn unchoke_peers(peers: &mut HashMap<usize, PeerState>) -> Vec<InternalEvent> {
    let mut output_events: Vec<InternalEvent> = vec![];

    let currently_unchoked: Vec<usize> = peers.iter()
        .filter(|(_idx, peer)| !peer.peer_choked_by_client)
        .map(|(idx, _peer)| *idx)
        .collect();

    let mut sorted_peers: Vec<&PeerState> = peers.values()
        .filter(|peer| peer.client_interested_in_peer)
        .collect();
    sorted_peers.sort_by(|a, b| b.downloaded_blocks.cmp(&a.downloaded_blocks));

    let top_peers: Vec<usize> = sorted_peers.iter()
        .take(MAX_CONCURRENTLY_UNCHOKED_PEERS)
        .map(|peer| peer.idx)
        .collect();

    for unchoked in currently_unchoked.iter() {
        if !top_peers.contains(&unchoked) {
            output_events.push(InternalEvent::ChokePeer(*unchoked));
        }
    }

    for top_peer in top_peers.iter() {
        if !currently_unchoked.contains(&top_peer) {
            output_events.push(InternalEvent::UnchokePeer(*top_peer));
        }
    }

    println!("Top peers:");
    for peer in top_peers {
        println!("{} with {}", peer, peers.get(&peer).unwrap().downloaded_blocks);
    }

    peers.into_iter().for_each(|(_idx, peer)| peer.downloaded_blocks = 0);

    return output_events;
}

fn optimistic_unchoke(peers: &mut HashMap<usize, PeerState>) -> Vec<InternalEvent> {
    return peers.iter()
        .filter(|(_idx, peer)| peer.is_unchokeable())
        .choose(&mut rand::thread_rng())
        .map_or_else(|| vec![], |peer| vec![InternalEvent::UnchokePeer(*peer.0)]);
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use crate::choke::handler::handle;
    use crate::choke::state::{ChokeEvent, PeerState};
    use crate::core_models::events::InternalEvent;

    #[test]
    fn test_handle_client_interested_in_peer() {
        let mut peers = init_peers(3);
        assert!(!peers.get(&1).unwrap().client_interested_in_peer);
        handle(ChokeEvent::ClientInterestedInPeer(1, true), &mut peers);
        assert!(peers.get(&1).unwrap().client_interested_in_peer);
    }

    #[test]
    fn test_handle_peer_interested_in_client() {
        let mut peers = init_peers(3);
        assert!(!peers.get(&1).unwrap().peer_interested_in_client);
        handle(ChokeEvent::PeerInterestedInClient(1, true), &mut peers);
        assert!(peers.get(&1).unwrap().peer_interested_in_client);
    }

    #[test]
    fn test_handle_block_downloaded_from_peer() {
        let mut peers = init_peers(3);
        assert_eq!(peers.get(&1).unwrap().downloaded_blocks, 0);
        handle(ChokeEvent::BlockDownloadedFromPeer(1), &mut peers);
        assert_eq!(peers.get(&1).unwrap().downloaded_blocks, 1);
    }

    #[test]
    fn test_handle_optimistic_unchoke() {
        let mut peers = init_peers(3);
        peers.get_mut(&0).unwrap().peer_choked_by_client = false;
        peers.get_mut(&1).unwrap().peer_choked_by_client = false;
        peers.get_mut(&2).unwrap().client_interested_in_peer = true;
        peers.get_mut(&2).unwrap().peer_interested_in_client = true;

        let result = handle(ChokeEvent::OptimisticUnchoke, &mut peers);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], InternalEvent::UnchokePeer(2));
    }

    #[test]
    fn test_handle_unchoke_peers() {
        let mut peers = init_peers(10);
        peers.iter_mut().for_each(|(_idx, peer)| {
            peer.client_interested_in_peer = true;
        });
        peers.get_mut(&0).unwrap().peer_choked_by_client = false;
        peers.get_mut(&1).unwrap().peer_choked_by_client = false;
        peers.get_mut(&2).unwrap().peer_choked_by_client = false;
        peers.get_mut(&3).unwrap().peer_choked_by_client = false;

        peers.get_mut(&0).unwrap().downloaded_blocks = 10;
        peers.get_mut(&1).unwrap().downloaded_blocks = 10;
        peers.get_mut(&4).unwrap().downloaded_blocks = 10;
        peers.get_mut(&5).unwrap().downloaded_blocks = 10;

        let result = handle(ChokeEvent::UnchokePeers, &mut peers);

        assert_eq!(result.len(), 4);
        assert!(result.contains(&InternalEvent::ChokePeer(2)));
        assert!(result.contains(&InternalEvent::ChokePeer(3)));
        assert!(result.contains(&InternalEvent::UnchokePeer(4)));
        assert!(result.contains(&InternalEvent::UnchokePeer(5)));
        assert!(peers.iter().all(|(_idx, peer)| peer.downloaded_blocks == 0));
    }

    fn init_peers(count: usize) -> HashMap<usize, PeerState> {
        return (0..count).into_iter()
            .map(|idx| (idx, PeerState::new(idx)))
            .collect();
    }
}