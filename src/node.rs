use crate::network::{Message::*, *};
use crate::protocols::bracha_broadcast::*;
use log::debug;
use std::hash::Hash;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::{self, JoinHandle};

pub type NodeId = usize;

// Faulty nodes stop sending message after FAULTY_AFTER messages
const FAULTY_AFTER: usize = 0;

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub(crate) enum Behaviour {
    Good,
    Faulty,
    Malicious,
}

const DEBUG_NODES: [NodeId; 2] = [0, 1];

// Struct to store parameters necessary for the network
pub(crate) struct Node {
    pub id: NodeId,
    pub behaviour: Behaviour,
    pub thread: JoinHandle<()>,
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
        let mut node = NodeInternals {
            id,
            behaviour: behaviour.clone(),
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
            .spawn(move || {
                let mut num_msg_received = 0;
                loop {
                    let msg = node.rx.recv().unwrap();
                    num_msg_received = num_msg_received + 1;
                    match node.handle_msg(msg, num_msg_received) {
                        // Continue processing message
                        ProtocolState::InProcess => (),

                        // Returns output of the protocol and terminate
                        ProtocolState::Terminated(v) => {
                            node.tx
                                .send(NetworkMessage::new(node.id, NETWORK_ID, END(v)));
                            break;
                        }

                        // Terminate the thread
                        ProtocolState::Interrupted => break,
                    }
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

pub(crate) enum ProtocolState {
    InProcess,
    Terminated(Value),
    Interrupted,
}

// Struct to store parameters necessary to do computations
pub(crate) struct NodeInternals {
    pub(crate) id: NodeId,
    pub(crate) behaviour: Behaviour,
    pub(crate) num_nodes: usize,
    pub(crate) max_faulty_nodes: usize,
    pub(crate) min_honnest_nodes: usize,
    pub(crate) neighbour_nodes: Vec<NodeId>,
    pub(crate) tx: Sender<NetworkMessage>,
    pub(crate) rx: Receiver<NetworkMessage>,

    pub(crate) bc_state: BroadcastState,
}

impl NodeInternals {
    /// Handle all the incoming messages
    /// Returns true to wait for new messages, false to terminate the node
    fn handle_msg(&mut self, msg: NetworkMessage, num_msg: usize) -> ProtocolState {
        match msg.msg {
            BROADCAST(bc_msg) => match self.behaviour {
                Behaviour::Good => handle_broadcast(self, msg.from, bc_msg),
                Behaviour::Faulty => {
                    if num_msg < FAULTY_AFTER {
                        return handle_broadcast(self, msg.from, bc_msg);
                    }
                    ProtocolState::InProcess
                }
                Behaviour::Malicious => ProtocolState::InProcess,
            },

            // Network asks the node to terminate
            END(_) => ProtocolState::Interrupted,
        }
    }


    pub(crate) fn send_to_all(&self, msg: Message) {
        for id in self.neighbour_nodes.iter() {
            self.tx
                .send(NetworkMessage::new(self.id, *id, msg.clone()));
        }
    }

    pub(crate) fn debug(&self) {
        if DEBUG_NODES.contains(&self.id) {
            debug!("NODE {}: {:?}", self.id, self.bc_state);
        }
    }
}

