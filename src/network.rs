use crate::node::*;
use crate::protocols::bracha_broadcast::BroadcastMessage;
use log::{debug, trace, warn};
use std::collections::HashMap;
use std::fmt;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time;

pub const NETWORK_ID: NodeId = 10000;
pub type Value = usize;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    BROADCAST(BroadcastMessage),

    // Sent by the network: node has to terminate
    // Sent by a node: protocol has finished and node delivers this value
    END(Value),
}
use Message::*;

unsafe impl Send for Message {}
unsafe impl Sync for Message {}

pub(crate) struct NetworkMessage {
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
    node_behaviours: HashMap<Behaviour, Vec<NodeId>>,
    rx: Receiver<NetworkMessage>,
    time_limit: Option<time::Duration>,
}

impl Network {
    /// Create new network with `num_nodes` nodes
    pub fn new(num_nodes: usize, num_malicious: usize, kind: MaliciousKind) -> Self {
        // Number of "bad" nodes shall be less than a third of the nodes
        assert!((num_malicious as f32) < (num_nodes as f32) / 3.0);

        let num_good = num_nodes - num_malicious;
        let mut nodes = HashMap::new();
        let (tx, network_rx): (Sender<NetworkMessage>, Receiver<NetworkMessage>) = channel();

        let mut good_nodes = vec![];
        let mut malicious_nodes = vec![];
        for id in 0..num_nodes {
            let (network_tx, rx): (Sender<NetworkMessage>, Receiver<NetworkMessage>) = channel();
            let mut neighbour_nodes = (0..num_nodes).collect::<Vec<NodeId>>();
            neighbour_nodes.remove(id);
            let behaviour = if id < num_good {
                good_nodes.push(id);
                Behaviour::Good
            } else {
                malicious_nodes.push(id);
                Behaviour::Malicious(kind.clone())
            };
            let node = Node::new(id, tx.clone(), rx, behaviour, neighbour_nodes);
            nodes.insert(id, (node, network_tx));
        }

        let mut node_behaviours = HashMap::new();
        node_behaviours.insert(Behaviour::Good, good_nodes);
        node_behaviours.insert(Behaviour::Malicious(kind), malicious_nodes);

        Network {
            num_nodes,
            nodes,
            node_behaviours,
            rx: network_rx,
            time_limit: None,
        }
    }

    pub fn bracha_broadcast(
        &mut self,
        v: Value,
        leader_node: NodeId,
    ) -> (bool, HashMap<NodeId, Value>) {
        // Start a broadcast
        if let Some((node, tx)) = self.nodes.get(&leader_node) {
            let bc_msg = Message::BROADCAST(BroadcastMessage::BC_LEADER(v));
            let msg = NetworkMessage::new(NETWORK_ID, node.id, bc_msg);
            trace!("{:?}", msg);
            tx.send(msg);
        }
        let results = self.run_network();

        // Termination: all honnest nodes have terminated
        let termination =
            results.len() == self.node_behaviours.get(&Behaviour::Good).unwrap().len();

        // Agreement: all honnest nodes output the same value
        let first = results.values().next().unwrap();
        let agreement = results.values().all(|res| res == first);

        // Validity: outputs of honnest nodes are equal to broadcasted value
        let validity = agreement && *first == v;

        (termination && agreement && validity, results)
    }

    fn run_network(&mut self) -> HashMap<NodeId, Value> {
        let mut good_running_nodes = self.node_behaviours.get(&Behaviour::Good).unwrap().len();
        let mut results = HashMap::new();
        loop {
            let network_msg = self.rx.recv().unwrap();
            match network_msg.msg {
                // Node has terminated and outputs v
                END(v) => {
                    let node_id = network_msg.from;

                    // Store result of the node
                    results.insert(node_id, v);

                    let (node, _) = self
                        .nodes
                        .remove(&node_id)
                        .expect("Can't terminate a node twice");

                    node.thread
                        .join()
                        .expect(&format!("oops, thread {} panicked", node.id));

                    if self
                        .node_behaviours
                        .get(&Behaviour::Good)
                        .unwrap()
                        .contains(&node_id)
                    {
                        // If a good node terminates
                        good_running_nodes = good_running_nodes - 1;

                        if good_running_nodes == 0 {
                            // If there are no more good nodes
                            // Send a termination message to all the bad nodes

                            warn!(
                                "Good nodes {:?} have terminated",
                                self.node_behaviours.get(&Behaviour::Good).unwrap()
                            );

                            // Send termination message to all bad nodes
                            for (node, tx) in self.nodes.values() {
                                tx.send(NetworkMessage::new(NETWORK_ID, node.id, END(0)));
                            }

                            // Wait for the bad nodes to end
                            for (node, _) in self.nodes.into_values() {
                                node.thread.join().unwrap();
                            }
                            break;
                        }
                    }
                }

                // Relay message to destination node
                _ => {
                    trace!("{:?}", network_msg);
                    if network_msg.to != NETWORK_ID {
                        if let Some((_, tx)) = self.nodes.get(&network_msg.to) {
                            // If the node is still up transmit the message
                            tx.send(network_msg);
                        } else {
                            warn!("Destination node is down: {:?}", network_msg);
                        }
                    }
                }
            }
        }
        results
    }

    pub fn close(self) 
}
