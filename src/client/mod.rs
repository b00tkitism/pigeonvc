use crate::protocol::{self, PacketType};
use anyhow::{self, Context};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;

pub mod ffi;

pub struct ClientCallbacks {
    pub on_joined: Option<fn(Vec<String>)>,
    pub on_talk: Option<fn(Vec<u8>)>,
    pub on_error: Option<fn(anyhow::Error)>,
}

impl Default for ClientCallbacks {
    fn default() -> Self {
        Self {
            on_joined: None,
            on_talk: None,
            on_error: None,
        }
    }
}

pub struct Client {
    sock: Arc<UdpSocket>,
    callbacks: Arc<RwLock<ClientCallbacks>>,
}

impl Client {
    pub async fn new(server_addr: String) -> anyhow::Result<Self> {
        let sock = UdpSocket::bind("0.0.0.0:0")
            .await
            .context("failed to bind local ephemeral UDP port")?;

        sock.connect(server_addr)
            .await
            .context("failed to connect udp socket")?;

        Ok(Self {
            sock: Arc::new(sock),
            callbacks: Arc::new(RwLock::new(ClientCallbacks::default())),
        })
    }

    pub async fn set_callbacks(&self, cb: ClientCallbacks) {
        let mut c = self.callbacks.write().await;
        *c = cb;
    }

    pub async fn validate_server(&self) -> anyhow::Result<bool> {
        self.sock.send(&protocol::new_ping()).await?;

        let mut buf = [0u8; 1500];
        let n = self.sock.recv(&mut buf).await?;

        if let Ok(pkt) = protocol::parse_from_server_packet(&buf[..n]) {
            match pkt {
                PacketType::Pong => return Ok(true),
                _ => anyhow::bail!("received unexpected packet during validation"),
            }
        }

        Ok(false)
    }

    pub async fn join(&self, name: String) -> anyhow::Result<()> {
        let pkt = protocol::new_join(name);
        self.sock.send(&pkt).await?;
        Ok(())
    }

    pub async fn send_audio(&self, pcm: &[u8]) -> anyhow::Result<()> {
        let pkt = protocol::new_talk(pcm);
        self.sock.send(&pkt).await?;
        Ok(())
    }

    pub async fn start_keepalive(&self) {
        let sock = self.sock.clone();

        tokio::spawn(async move {
            loop {
                let _ = sock.send(&protocol::new_alive()).await;
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        });
    }

    pub async fn start_listener(&self) {
        let sock = self.sock.clone();
        let callbacks = self.callbacks.clone();

        tokio::spawn(async move {
            let mut buf = [0u8; 1500];

            loop {
                let pkt = match sock.recv(&mut buf).await {
                    Ok(n) => protocol::parse_from_server_packet(&buf[..n]),
                    Err(e) => {
                        if let Some(on_err) = callbacks.read().await.on_error {
                            on_err(anyhow::Error::msg(format!("recv error: {e}")));
                        }
                        continue;
                    }
                };

                match pkt {
                    Err(e) => {
                        if let Some(on_err) = callbacks.read().await.on_error {
                            on_err(e);
                        }
                    }

                    Ok(PacketType::Joined { users }) => {
                        if let Some(func) = callbacks.read().await.on_joined {
                            func(users);
                        }
                    }

                    Ok(PacketType::Talked { audio_data }) => {
                        if let Some(func) = callbacks.read().await.on_talk {
                            func(audio_data);
                        }
                    }

                    _ => {}
                }
            }
        });
    }
}
