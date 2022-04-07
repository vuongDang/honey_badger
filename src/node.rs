use crate::network::{Message::*, *};
use log::debug;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::{self, JoinHandle};

pub type NodeId = usize;
pub enum Behaviour {
    Good,
    Faulty,
    Malicious,
}

const DEBUG_NODES: [NodeId; 2] = [0, 1];

// Struct to store parameters necessary for the network
pub struct Node {
    pub id: NodeId,
    pub behaviour: Behaviour,
    pub thread: JoinHandle<()>,
}

// Struct to store parameters necessary to do computations
struct NodeState {
    id: NodeId,
    num_nodes: usize,
    max_faulty_nodes: usize,
    min_honnest_nodes: usize,
    neighbour_nodes: Vec<NodeId>,
    tx: Sender<NetworkMessage>,
    rx: Receiver<NetworkMessage>,

    bc_state: BroadcastState,
}

impl NodeState {
    /// Handle all the incoming messages
    /// Returns true to wait for new messages, false to terminate the node
    fn handle_msg(&mut self, msg: NetworkMessage) -> bool {
        match msg.msg {
            VALUE(_v) => {
                ()
                // println!("[THREAD {}][FROM THREAD:{}]: {:?}", self.id, msg.from, v)
            }

            BROADCAST(bc_msg) => {
                if !self.handle_broadcast(msg.from, bc_msg) {
                    return false;
                }
            }

            // Network asks the node to terminate
            // Confirm the message and terminate
            END => {
                self.tx.send(NetworkMessage::new(self.id, NETWORK_ID, END));
                return false;
            }
        }
        return true;
    }

    /// Handle messages related to broadcast
    /// Returns true if protocol has not terminated yet, false otherwise
    fn handle_broadcast(&mut self, from: NodeId, msg: BroadcastMessage) -> bool {
        match msg {
            // Node has been chosen as an initiator for broadcast
            BC_LEADER(v) => {
                self.send_bc_to_all(BC_INIT(v));
                self.send_bc_to_all(BC_ECHO(v));
                self.bc_state.echo = false;
            }

            // Initiator node has initiated a broadcast
            BC_INIT(v) => {
                if self.bc_state.echo {
                    // We haven't sent ECHO yet
                    self.send_bc_to_all(BC_ECHO(v));
                    self.bc_state.echo = false;
                }
            }

            // Sender node have received a value from the initiator node
            BC_ECHO(v) => {
                // First ECHO with this value v received
                if self.bc_state.echo_received.get(&v).is_none() {
                    // Init hashset for value v
                    self.bc_state.echo_received.insert(v, HashSet::new());
                }

                let echo_v_received = self.bc_state.echo_received.get_mut(&v).unwrap();
                // Add sender node to list of nodes who sent <ECHO, v>
                echo_v_received.insert(from);

                if self.bc_state.ready {
                    // We haven't sent READY yet
                    if echo_v_received.len() >= self.min_honnest_nodes {
                        // Potentially we have received ECHO from all the honnest
                        // nodes and might not receive any more ECHO messages

                        self.send_bc_to_all(BC_READY(v));
                        // Init hashset for value v
                        self.bc_state.ready_received.insert(v, HashSet::new());
                        self.bc_state.ready = false
                    }
                }
                self.debug();
            }

            // Sender node know that other nodes have also received a
            // value from the initiator
            BC_READY(v) => {
                // First <READY, v> received
                if self.bc_state.ready_received.get(&v).is_none() {
                    // Init hashset for value v
                    self.bc_state.ready_received.insert(v, HashSet::new());
                }

                let ready_v_received = self.bc_state.ready_received.get_mut(&v).unwrap();
                ready_v_received.insert(from);

                if self.bc_state.ready {
                    // We haven't sent READY yet

                    if ready_v_received.len() > self.max_faulty_nodes {
                        // At least one of the READY comes from an honnest node

                        self.send_bc_to_all(BC_READY(v));
                        self.bc_state.ready = false
                    }
                    self.debug();
                } else if self.bc_state.ready_received.get(&v).unwrap().len()
                    >= self.min_honnest_nodes
                {
                    // We have received <READY, v> from potentially all the honnest
                    // nodes and we might not receive more READY messages
                    self.tx.send(NetworkMessage::new(
                        self.id,
                        NETWORK_ID,
                        BROADCAST(BC_OUTPUT(v)),
                    ));
                    return false;
                }
            }

            // This message is used to signal the network has finished.
            // A node should not receive this message.
            BC_OUTPUT(_) => unreachable!("Node should not receive this message"),
        }
        true
    }

    fn send_bc_to_all(&self, msg: BroadcastMessage) {
        for id in self.neighbour_nodes.iter() {
            self.tx
                .send(NetworkMessage::new(self.id, *id, BROADCAST(msg.clone())));
        }
    }

    fn debug(&self) {
        if DEBUG_NODES.contains(&self.id) {
            debug!("NODE {}: {:?}", self.id, self.bc_state);
        }
    }
}

impl Node {
    pub fn new(
        id: NodeId,
        tx: Sender<NetworkMessage>,
        rx: Receiver<NetworkMessage>,
        behaviour: Behaviour,
        neighbour_nodes: Vec<NodeId>,
    ) -> Node {
        // Parameters
        let num_nodes = neighbour_nodes.len() + 1;
        // Number of faulty nodes must be inferior to 1/3
        let max_faulty_nodes = num_nodes / 3;
        let min_honnest_nodes = num_nodes - max_faulty_nodes;
        let mut node = NodeState {
            id,
            num_nodes,
            max_faulty_nodes,
            min_honnest_nodes,
            neighbour_nodes,
            tx,
            rx,
            bc_state: BroadcastState::new(),
        };

        // Start thread to handle all the node computations
        let thread = thread::Builder::new()
            .name(format!("Node {}", id))
            .spawn(move || loop {
                let msg = node.rx.recv().unwrap();
                if !node.handle_msg(msg) {
                    // When node is done with its task
                    node.tx.send(NetworkMessage::new(node.id, NETWORK_ID, END));
                    break;
                }
            })
            .expect(&format!("Could not spawn thread {}", id));
        Node {
            id,
            behaviour,
            thread,
        }
    }
}

// BROADCAST
#[derive(Debug)]
struct BroadcastState {
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
pub enum BroadcastMessage {
    BC_LEADER(Value),
    BC_INIT(Value),
    BC_ECHO(Value),
    BC_READY(Value),
    BC_OUTPUT(Value),
}
use BroadcastMessage::*;

impl fmt::Debug for BroadcastMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            BC_LEADER(v) => format!("<LEADER, {}>", v),
            BC_INIT(v) => format!("<INIT, {}>", v),
            BC_ECHO(v) => format!("<ECHO, {}>", v),
            BC_READY(v) => format!("<READY, {}>", v),
            BC_OUTPUT(v) => format!("<OUTPUT, {}>", v),
        };
        write!(f, "{}", s)
    }
}
