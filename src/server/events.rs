// src/server/events.rs
use std::net::SocketAddr;
use std::sync::Arc;

use crate::server::Server;
use crate::server::model::{StoredEvent, User};

use super::model::{MAX_CONSECUTIVE_BEHIND, MAX_EVENT_HISTORY};

impl Server {
    pub async fn broadcast_event(
        &self,
        pkt_builder: impl Fn(u64) -> Vec<u8>,
        recipients: &[SocketAddr],
    ) {
        if recipients.is_empty() {
            return;
        }

        let mut event_system = self.event_system.write().await;

        let seq = event_system.next_seq;
        event_system.next_seq += 1;

        let pkt = pkt_builder(seq);

        if event_system.history.len() == MAX_EVENT_HISTORY {
            event_system.history.pop_front();
        }
        event_system.history.push_back(StoredEvent {
            seq,
            data: pkt.clone(),
        });

        drop(event_system);

        self.batch_send(&pkt, recipients).await;
    }

    pub async fn handle_alive_sync(&self, addr: SocketAddr, user_arc: Arc<User>, client_seq: u64) {
        enum SyncAction {
            UpToDate,
            Resend(Vec<Vec<u8>>),
            Disconnect(String),
        }

        let action = {
            let event_system = self.event_system.read().await;
            let server_last_seq = event_system.next_seq.saturating_sub(1);

            if client_seq >= server_last_seq {
                SyncAction::UpToDate
            } else {
                let behind_by = server_last_seq - client_seq;

                if behind_by > MAX_EVENT_HISTORY as u64 {
                    SyncAction::Disconnect(format!(
                        "Sync failure: Too far behind ({} events)",
                        behind_by
                    ))
                } else {
                    let failures = user_arc
                        .consecutive_behind
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                        + 1;
                    if failures >= MAX_CONSECUTIVE_BEHIND {
                        SyncAction::Disconnect(format!(
                            "Sync failure: Behind {} consecutive times",
                            failures
                        ))
                    } else {
                        let mut packets_to_resend = Vec::with_capacity(behind_by as usize);
                        for event in event_system.history.iter() {
                            if event.seq > client_seq {
                                packets_to_resend.push(event.data.clone());
                            }
                        }
                        if packets_to_resend.len() as u64 != behind_by {
                            SyncAction::Disconnect(
                                "Internal server error: Event history inconsistency".to_string(),
                            )
                        } else {
                            SyncAction::Resend(packets_to_resend)
                        }
                    }
                }
            }
        };

        match action {
            SyncAction::UpToDate => {
                if user_arc
                    .consecutive_behind
                    .load(std::sync::atomic::Ordering::Relaxed)
                    > 0
                {
                    user_arc
                        .consecutive_behind
                        .store(0, std::sync::atomic::Ordering::Relaxed);
                }
            }
            SyncAction::Resend(packets) => {
                println!("User {addr} is behind. Resending {} events.", packets.len());
                for pkt in packets {
                    let _ = self.listener.send_to(&pkt, addr).await;
                }
            }
            SyncAction::Disconnect(reason) => {
                println!("Disconnecting user {addr}: {}", reason);
                self.disconnect_user(addr, Some(&reason)).await;
            }
        }
    }
}
