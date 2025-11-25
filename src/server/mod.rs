use crate::protocol::PacketType;

use super::protocol;
use anyhow;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{net::UdpSocket, sync::RwLock};

pub struct Server {
    listener: Arc<UdpSocket>,
    users: RwLock<HashMap<SocketAddr, (String, AtomicU64)>>,
}

impl Server {
    pub async fn new(listen_addr: String) -> anyhow::Result<Server> {
        let listener = UdpSocket::bind(listen_addr).await?;
        Ok(Self {
            listener: Arc::new(listener),
            users: RwLock::new(HashMap::new()),
        })
    }

    pub async fn routine(self: Arc<Self>) {
        loop {
            let users_snapshot: HashMap<SocketAddr, u64> = {
                let users = self.users.read().await;
                users
                    .iter()
                    .map(|(k, v)| (*k, v.1.load(Ordering::Relaxed)))
                    .collect()
            };
            for user in users_snapshot {
                if SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    > user.1
                {
                    {
                        self.users.write().await.remove(&user.0);
                    }
                    self.send_to_all(&protocol::new_joined(self.joined_users().await), None)
                        .await;
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        }
    }

    pub async fn listen(self: Arc<Self>) {
        loop {
            let mut buf: [u8; 1500] = [0; 1500];
            let (n, addr) = match self.listener.recv_from(&mut buf).await {
                Err(_) => continue,
                Ok(v) => v,
            };

            let packet = buf[..n].to_vec();
            let this = self.clone();

            tokio::spawn(async move {
                if let Err(e) = this.handle(addr, &packet).await {
                    eprintln!("handle error: {e}");
                }
            });
        }
    }

    async fn handle(&self, addr: SocketAddr, buf: &[u8]) -> anyhow::Result<()> {
        let packet_type = protocol::parse_from_client_packet(buf)?;

        match packet_type {
            PacketType::Ping => {
                let _ = self.listener.send_to(&protocol::new_pong(), addr).await?;
            }
            PacketType::Join { name } => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                {
                    let mut users = self.users.write().await;
                    match users.get(&addr) {
                        None => {
                            users.insert(addr, (name.clone(), AtomicU64::new(now + 1)));
                            println!("[JOIN] {} from {}", name, addr);
                            {}
                        }
                        _ => {}
                    };
                }

                self.send_to_all(&protocol::new_joined(self.joined_users().await), None)
                    .await;
            }
            PacketType::Alive => {
                self.keepalive(addr).await;
                let _ = self.listener.send_to(&protocol::new_alived(), addr).await;
            }
            PacketType::Talk { audio_data } => {
                self.keepalive(addr).await;
                self.send_to_all(&protocol::new_talked(&audio_data), Some(addr))
                    .await;
            }
            _ => {}
        };

        Ok(())
    }

    async fn keepalive(&self, addr: SocketAddr) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 1;

        if let Some((_, ts)) = self.users.read().await.get(&addr) {
            ts.store(now, Ordering::Relaxed);
        }
    }

    async fn joined_users(&self) -> Vec<String> {
        let mut users_list = vec![];

        let users = self.users.read().await;
        for (name, _) in users.values() {
            users_list.push(name.clone());
        }

        users_list
    }

    async fn send_to_all(&self, buf: &[u8], expect_addr: Option<SocketAddr>) {
        let users = self.users.read().await;

        let addrs = users.keys().cloned().filter(|addr| match expect_addr {
            Some(e) => *addr != e,
            None => true,
        });

        for addr in addrs {
            let _ = self.listener.send_to(buf, addr).await;
        }
    }
}
