// src/server/net.rs
use std::net::SocketAddr;

use crate::server::Server;

impl Server {
    pub async fn listen(&self) {
        loop {
            let mut buf = [0u8; 1500];
            let Ok((n, addr)) = self.listener.recv_from(&mut buf).await else {
                continue;
            };
            let _ = self.handle(addr, &buf[..n]).await;
        }
    }

    pub async fn batch_send(&self, buf: &[u8], addrs: &[SocketAddr]) {
        for addr in addrs.iter() {
            let _ = self.listener.send_to(buf, addr).await;
        }
    }

    pub async fn batch_send_room(&self, buf: &[u8], room_id: u16, except: Option<SocketAddr>) {
        let Some(room_arc) = self.rooms.get(&room_id).map(|r| r.value().clone()) else {
            return;
        };
        let addrs = room_arc.addr_list.read().await;
        match except {
            Some(skip) => {
                for addr in addrs.iter() {
                    if *addr != skip {
                        let _ = self.listener.send_to(buf, addr).await;
                    }
                }
            }
            None => {
                for addr in addrs.iter() {
                    let _ = self.listener.send_to(buf, addr).await;
                }
            }
        }
    }
}
