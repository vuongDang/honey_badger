use crate::node::*;
use log::{trace, warn};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time;
use std::fmt;

pub const NETWORK_ID: NodeId = 10000;
pub type Value = usize;

#[derive(Debug)]
pub enum Message {
    VALUE(Value),
    BROADCAST(BroadcastMessage),
    END,
}
use Message::*;

unsafe impl Send for Message {}
unsafe impl Sync for Message {}

pub struct NetworkMessage {
    pub from: NodeId,
    pub to: NodeId,
    pub msg: Message,
}

impl fmt::Debug for NetworkMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let from = if self.from == NETWORK_ID {
            String::from("Network")
        } else {
            format!("{:?}", self.from)
        };

        let to = if self.to == NETWORK_ID {
            String::from("Network")
        } else {
            format!("{:?}", self.to)
        };

        write!(f, "[{} -> {}] {:?}", from, to, self.msg)
    }
}

impl NetworkMessage {
    pub fn new(from: NodeId, to: NodeId, msg: Message) -> Self {
        NetworkMessage { from, to, msg }
    }
}

unsafe impl Send for NetworkMessage {}
unsafe impl Sync for NetworkMessage {}

pub struct Network {
    num_nodes: usize,
    // Nodes of the network, node id corresponds to its index
    nodes: HashMap<NodeId, (Node, Sender<NetworkMessage>)>,
    rx: Receiver<NetworkMessage>,
    time_limit: Option<time::Duration>,
}

impl Network {
    /// Create new network with `num_nodes` nodes
    pub fn new(num_nodes: usize) -> Self {

        let mut nodes = HashMap::new();
        let (tx, network_rx): (Sender<NetworkMessage>, Receiver<NetworkMessage>) = channel();

        for id in 0..num_nodes {
            let (network_tx, rx): (Sender<NetworkMessage>, Receiver<NetworkMessage>) = channel();
            let mut neighbour_nodes = (0..num_nodes).collect::<Vec<NodeId>>();
            neighbour_nodes.remove(id);
            let node = Node::new(id, tx.clone(), rx, Behaviour::Good, neighbour_nodes);
            nodes.insert(id, (node, network_tx));
        }
        Network {
            num_nodes,
            nodes,
            rx: network_rx,
            time_limit: None,
        }
    }

    pub fn run(mut self) {
        // Start a broadcast
        let v = 7;
        let leader_node = 0;
        if let Some((node, tx)) = self.nodes.get(&leader_node) {
            let bc_msg = Message::BROADCAST(BroadcastMessage::BC_LEADER(v));
            let msg = NetworkMessage::new(NETWORK_ID, node.id, bc_msg);
            trace!("{:?}", msg);
            tx.send(msg);
        }

        let mut num_running_nodes = self.num_nodes;
        loop {
            let network_msg = self.rx.recv().unwrap();
            match network_msg.msg {
                // Node has terminated
                END => {
                    let node_id = network_msg.from;
                    let (node, _) = self
                        .nodes
                        .remove(&node_id)
                        .expect("Can't terminate a node twice");
                    node.thread
                        .join()
                        .expect(&format!("oops, thread {} panicked", node.id));

                    warn!("Thread {} has terminated", node.id);
                    num_running_nodes = num_running_nodes - 1;
                    if num_running_nodes == 0 {
                        break;
                    }
                }

                // Relay message to destination node
                _ => {
                    trace!("{:?}", network_msg);
                    if network_msg.to != NETWORK_ID {
                        let (_, tx) = self.nodes.get(&network_msg.to).expect(&format!(
                            "[NETWORK ERROR] Trying to send message to terminated node.\n\t{:?}",
                            network_msg
                        ));
                    tx.send(network_msg);
                    }
                }
            }
        }
    }
}
