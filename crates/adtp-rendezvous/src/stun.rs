pub struct StunServer;

impl StunServer {
    pub fn reflect_external_addr(&self, peer_addr: &std::net::SocketAddr) -> String {
        peer_addr.to_string()
    }
}
