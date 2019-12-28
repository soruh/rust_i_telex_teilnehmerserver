use models::ProcessedPackage5;
use std::net::SocketAddr;

pub struct Server {
    queue: Vec<ProcessedPackage5>,
    address: SocketAddr,
}