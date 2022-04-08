use distributed::network::Network;
use log::{trace, warn};

fn main() {
    pretty_env_logger::init();
    trace!("Starting...");
    let mut network = Network::new(10, 3, 0);
    trace!("Network created...");
    let (success, results) = network.bracha_broadcast(7, 0);
    if success {
        trace!("Bracha broadcast successful: {:?}", results)
    } else {
        warn!("Bracha broadcast failed: {:?}", results)
    }
}
