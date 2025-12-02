// src/server/model.rs
use dashmap::DashMap;
use std::collections::VecDeque;
use std::pin::Pin;
use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicU8, AtomicU16, AtomicU64},
    },
};
use tokio::net::UdpSocket;
use tokio::sync::RwLock;

pub const USER_TIMEOUT_SECS: u64 = 5;
pub const ROUTINE_SLEEP_MS: u64 = 500;
pub const MAX_EVENT_HISTORY: usize = 100;
pub const MAX_CONSECUTIVE_BEHIND: u8 = 3;

pub struct User {
    pub id: u64,
    pub name: String,
    pub last_seen: AtomicU64,
    pub room_id: AtomicU16,
    pub flags: u8,
    pub consecutive_behind: AtomicU8,
}

pub struct Room {
    pub name: String,
    pub users: DashMap<SocketAddr, Arc<User>>,
    pub joined_snapshot: RwLock<Vec<(u64, String)>>,
    pub addr_list: RwLock<Vec<SocketAddr>>,
}

#[derive(Clone)]
pub struct StoredEvent {
    pub seq: u64,
    pub data: Vec<u8>,
}

pub struct EventSystem {
    pub next_seq: u64,
    pub history: VecDeque<StoredEvent>,
}

type TryJoinFn = Arc<
    dyn Fn(String) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>>
        + Send
        + Sync,
>;

pub struct Server {
    pub(crate) listener: Arc<UdpSocket>,
    pub(crate) rooms: DashMap<u16, Arc<Room>>,
    pub(crate) users: DashMap<SocketAddr, Arc<User>>,
    pub(crate) connected_addrs: RwLock<Vec<SocketAddr>>,
    pub(crate) next_user_id: AtomicU64,
    pub(crate) event_system: RwLock<EventSystem>,
    pub(crate) try_join: TryJoinFn,
}

impl Server {
    pub async fn new<F, FR>(listen_addr: String, join_fn: F) -> anyhow::Result<Self>
    where
        F: Fn(String) -> FR + Send + Sync + 'static,
        FR: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        let listener = Arc::new(UdpSocket::bind(listen_addr).await?);

        let try_join: TryJoinFn = Arc::new(move |hwid: String| Box::pin(join_fn(hwid)));

        let server = Self {
            listener,
            rooms: DashMap::new(),
            users: DashMap::new(),
            connected_addrs: RwLock::new(Vec::new()),
            next_user_id: AtomicU64::new(1),
            event_system: RwLock::new(EventSystem {
                next_seq: 1,
                history: VecDeque::with_capacity(MAX_EVENT_HISTORY),
            }),
            try_join,
        };

        Ok(server)
    }

    pub fn add_room_with_id(&self, id: u16, name: &str) {
        self.rooms.insert(id, Self::make_room(name));
    }

    fn make_room(name: &str) -> Arc<Room> {
        Arc::new(Room {
            name: name.to_string(),
            users: DashMap::new(),
            joined_snapshot: RwLock::new(Vec::new()),
            addr_list: RwLock::new(Vec::new()),
        })
    }
}
