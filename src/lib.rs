#![allow(unused_must_use)]
#![allow(non_camel_case_types)]
#![allow(dead_code)]
pub mod network;
pub mod node;
pub mod protocols;


#[cfg(test)]
mod tests {
    use crate::network::Network;
    #[test]
    fn it_works() {
        let network = Network::new(10);
        network.run();
    }
}
