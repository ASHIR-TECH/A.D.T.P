use crate::agent_card::AgentCard;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct AgentCardServer {
    port: u16,
    card: Arc<RwLock<AgentCard>>,
}

impl AgentCardServer {
    pub fn new(port: u16, card: AgentCard) -> Self {
        Self {
            port,
            card: Arc::new(RwLock::new(card)),
        }
    }

    pub async fn update_card(&self, new_card: AgentCard) {
        let mut card = self.card.write().await;
        *card = new_card;
    }

    pub async fn get_card_json(&self) -> String {
        let card = self.card.read().await;
        serde_json::to_string_pretty(&*card).unwrap_or_default()
    }

    fn handle_client(mut stream: TcpStream, card_json: &str, url: &str) {
        if url == "/.well-known/adtp-agent.json" {
            let body = card_json.as_bytes();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(body);
        } else {
            let body = b"Not Found";
            let response = format!(
                "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(body);
        }
    }

    pub async fn serve(self) {
        let addr = format!("127.0.0.1:{}", self.port);

        let listener = match TcpListener::bind(&addr) {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("Failed to bind Agent Card HTTP server on {addr}: {e}");
                return;
            }
        };

        listener
            .set_nonblocking(true)
            .expect("Cannot set non-blocking");

        tracing::info!("Agent Card HTTP server listening on {}", addr);

        loop {
            match listener.accept() {
                Ok((stream, _peer_addr)) => {
                    let card = self.card.clone();
                    tokio::task::spawn_blocking(move || {
                        let mut buf = [0u8; 4096];
                        let mut read_stream = stream.try_clone().expect("clone failed");
                        if read_stream.read(&mut buf).is_ok() {
                            let request_str = String::from_utf8_lossy(&buf);
                            let url = request_str
                                .lines()
                                .next()
                                .and_then(|l| l.split_whitespace().nth(1))
                                .unwrap_or("/");
                            let url = url.to_string();

                            let card_json = {
                                let rt = tokio::runtime::Handle::try_current();
                                match rt {
                                    Ok(handle) => {
                                        let card = card.clone();
                                        tokio::task::block_in_place(|| {
                                            handle.block_on(async {
                                                let c = card.read().await;
                                                serde_json::to_string_pretty(&*c)
                                                    .unwrap_or_default()
                                            })
                                        })
                                    }
                                    Err(_) => "{}".to_string(),
                                }
                            };

                            Self::handle_client(stream, &card_json, &url);
                        }
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(e) => {
                    tracing::error!("Agent Card server accept error: {e}");
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }
    }
}
