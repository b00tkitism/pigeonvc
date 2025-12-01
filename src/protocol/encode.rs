// src/protocol/encode.rs
use crate::protocol::constants::*;

pub fn new_accepted(seq: u64, user_id: u64) -> Vec<u8> {
    let mut packet = MAGIC.to_vec();
    packet.extend_from_slice(&ACCEPTED.to_be_bytes());
    packet.extend_from_slice(&seq.to_be_bytes());
    packet.extend_from_slice(&user_id.to_be_bytes());
    packet
}

pub fn new_ping() -> Vec<u8> {
    let mut packet = MAGIC.to_vec();
    packet.extend_from_slice(&PING.to_be_bytes());
    packet
}

pub fn new_pong() -> Vec<u8> {
    let mut packet = MAGIC.to_vec();
    packet.extend_from_slice(&PONG.to_be_bytes());
    packet
}

pub fn new_rooms_list(remaining: bool, list: Vec<(u16, String)>) -> Vec<u8> {
    let mut packet = MAGIC.to_vec();
    packet.extend_from_slice(&ROOMSLIST.to_be_bytes());
    packet.push(remaining.into());
    for room in list.iter() {
        packet.extend_from_slice(&room.0.to_be_bytes());
        packet.extend_from_slice(room.1.as_bytes());
        packet.push(0);
    }
    packet
}

pub fn new_event(seq: u64, joined: bool, room_id: u16, user_id: u64, name: &str) -> Vec<u8> {
    let mut packet = MAGIC.to_vec();
    packet.extend_from_slice(&EVENT.to_be_bytes());
    packet.extend_from_slice(&seq.to_be_bytes());
    packet.extend_from_slice(&room_id.to_be_bytes());
    packet.extend_from_slice(&user_id.to_be_bytes());
    packet.extend_from_slice(name.as_bytes());
    packet.push(0);
    packet.push(joined.into());
    packet
}

pub fn new_join(name: &str) -> Vec<u8> {
    let mut packet = MAGIC.to_vec();
    packet.extend_from_slice(&JOIN.to_be_bytes());
    packet.extend_from_slice(name.as_bytes());
    packet.push(0);
    packet
}

pub fn new_joined(room_id: u16, users: Vec<(u64, String)>) -> Vec<u8> {
    let mut packet = MAGIC.to_vec();
    packet.extend_from_slice(&JOINED.to_be_bytes());
    packet.extend_from_slice(&room_id.to_be_bytes());
    for user in users {
        packet.extend_from_slice(&user.0.to_be_bytes());
        packet.extend_from_slice(user.1.as_bytes());
        packet.push(0);
    }
    packet
}

pub fn new_talk(audio_data: &[u8]) -> Vec<u8> {
    let mut packet = MAGIC.to_vec();
    packet.extend_from_slice(&TALK.to_be_bytes());
    packet.extend_from_slice(audio_data);
    packet
}

pub fn new_talked_audio(talker: u64, audio_data: &[u8]) -> Vec<u8> {
    let mut packet = MAGIC.to_vec();
    packet.extend_from_slice(&TALKED.to_be_bytes());
    packet.push(0);
    packet.extend_from_slice(&talker.to_be_bytes());
    packet.extend_from_slice(audio_data);
    packet
}

pub fn new_alive() -> Vec<u8> {
    let mut packet = MAGIC.to_vec();
    packet.extend_from_slice(&ALIVE.to_be_bytes());
    packet
}

pub fn new_alived() -> Vec<u8> {
    let mut packet = MAGIC.to_vec();
    packet.extend_from_slice(&ALIVED.to_be_bytes());
    packet
}

pub fn new_disconnect(reason: &str) -> Vec<u8> {
    let mut packet = MAGIC.to_vec();
    packet.extend_from_slice(&DISCONNECT.to_be_bytes());
    packet.extend_from_slice(reason.as_bytes());
    packet.push(0);
    packet
}
