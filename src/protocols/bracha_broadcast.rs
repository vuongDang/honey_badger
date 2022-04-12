use crate::network::{Message::*, *};
use crate::node::*;
use std::collections::{HashMap, HashSet};
use std::fmt;
use rand::{
    distributions::{Distribution, Standard},
    Rng,
};

#[derive(Debug)]
pub(crate) struct BroadcastState {
    echo: bool,
    ready: bool,
    echo_received: HashMap<Value, HashSet<NodeId>>,
    ready_received: HashMap<Value, HashSet<NodeId>>,
}

impl BroadcastState {
    pub fn new() -> Self {
        BroadcastState {
            echo: true,
            ready: true,
            echo_received: HashMap::new(),
            ready_received: HashMap::new(),
        }
    }
}

#[derive(Clone)]
pub(crate) enum BroadcastMessage {
    BC_LEADER(Value),
    BC_INIT(Value),
    BC_ECHO(Value),
    BC_READY(Value),
}
use BroadcastMessage::*;

impl BroadcastMessage {
    pub(crate) fn malicious(&self) -> Self {
        match self {
            BC_LEADER(_) => BC_LEADER(MALICIOUS_VALUE),
            BC_INIT(_) => BC_INIT(MALICIOUS_VALUE),
            BC_ECHO(_) =>BC_ECHO(MALICIOUS_VALUE),
            BC_READY(_) => BC_ECHO(MALICIOUS_VALUE),
        }
    }
}

/// Handle messages related to broadcast
pub(crate) fn handle_broadcast(
    node: &mut NodeInternals,
    from: NodeId,
    msg: BroadcastMessage,
) -> ProtocolState {
    match msg {
        // Node has been chosen as an initiator for broadcast
        BC_LEADER(v) => {
            node.send_to_all(BROADCAST(BC_INIT(v)));
            node.send_to_all(BROADCAST(BC_ECHO(v)));
            node.bc_state.echo = false;
        }

        // Initiator node has initiated a broadcast
        BC_INIT(v) => {
            if node.bc_state.echo {
                // We haven't sent ECHO yet
                node.send_to_all(BROADCAST(BC_ECHO(v)));
                node.bc_state.echo = false;
            }
        }

        // Sender node have received a value from the initiator node
        BC_ECHO(v) => {
            // First ECHO with this value v received
            if node.bc_state.echo_received.get(&v).is_none() {
                // Init hashset for value v
                node.bc_state.echo_received.insert(v, HashSet::new());
            }

            let echo_v_received = node.bc_state.echo_received.get_mut(&v).unwrap();
            // Add sender node to list of nodes who sent <ECHO, v>
            echo_v_received.insert(from);

            if node.bc_state.ready {
                // We haven't sent READY yet
                if echo_v_received.len() >= (node.min_honnest_nodes - 1) {
                    // Potentially we have received ECHO from all the honnest
                    // nodes and might not receive any more ECHO messages
                    // -1 because we don't send msg to ourselved

                    node.send_to_all(BROADCAST(BC_READY(v)));
                    // Init hashset for value v
                    node.bc_state.ready_received.insert(v, HashSet::new());
                    node.bc_state.ready = false
                }
            }
            node.debug();
        }

        // Sender node know that other nodes have also received a
        // value from the initiator
        BC_READY(v) => {
            // First <READY, v> received
            if node.bc_state.ready_received.get(&v).is_none() {
                // Init hashset for value v
                node.bc_state.ready_received.insert(v, HashSet::new());
            }

            let ready_v_received = node.bc_state.ready_received.get_mut(&v).unwrap();
            ready_v_received.insert(from);

            if node.bc_state.ready {
                // We haven't sent READY yet

                if ready_v_received.len() > node.max_malicious_nodes {
                    // At least one of the READY comes from an honnest node

                    node.send_to_all(BROADCAST(BC_READY(v)));
                    node.bc_state.ready = false
                }
                node.debug();
            } else if node.bc_state.ready_received.get(&v).unwrap().len()
                >= node.min_honnest_nodes - 1
            {
                return ProtocolState::Terminated(v);
            }
        }
    }
    ProtocolState::InProcess
}


/// Malicious node tries to corrupt the broadcast to
/// the value MALICIOUS_VALUE by
/// sending random ECHO and READY messages
pub(crate) fn random_broadcast(
    node: &mut NodeInternals,
    _from: NodeId,
    _msg: BroadcastMessage,
) -> ProtocolState {
    let random_msg: BroadcastMessage = rand::random();
    node.send_to_all(BROADCAST(random_msg));
    ProtocolState::InProcess
}


impl fmt::Debug for BroadcastMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            BC_LEADER(v) => format!("<LEADER, {}>", v),
            BC_INIT(v) => format!("<INIT, {}>", v),
            BC_ECHO(v) => format!("<ECHO, {}>", v),
            BC_READY(v) => format!("<READY, {}>", v),
        };
        write!(f, "{}", s)
    }
}


impl Distribution<BroadcastMessage> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> BroadcastMessage {
        match rng.gen_range(0..2usize) { // rand 0.8
            0 => BC_ECHO(MALICIOUS_VALUE),
            _ => BC_READY(MALICIOUS_VALUE),
        }
    }
}
