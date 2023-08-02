use crate::transfer::peer_connection::PeerConnection;
use crate::transfer::state::PeerTransferState;

#[derive(Debug)]
pub enum P2PTransferError {}

pub async fn run_transfer(state: PeerTransferState, conn: PeerConnection) -> Result<(), P2PTransferError> {
    println!("running transfer with peer!");
    return Ok(());
}