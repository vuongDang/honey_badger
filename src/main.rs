use distributed::network::Network;
use log::trace;

fn main() {
    pretty_env_logger::init();
    println!("Starting...");
    let network = Network::new(10);
    trace!("Network created...");
    network.run();
}
