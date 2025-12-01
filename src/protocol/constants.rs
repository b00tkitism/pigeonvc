// src/protocol/constants.rs
pub const MAGIC: [u8; 4] = [0xde, 0xad, 0xc0, 0xde];

pub const PING: u32 = 1;
pub const PONG: u32 = 2;
pub const JOIN: u32 = 3;
pub const JOINED: u32 = 4;
pub const TALK: u32 = 5;
pub const TALKED: u32 = 6;
pub const ALIVE: u32 = 7;
pub const ALIVED: u32 = 8;
pub const ROOMS: u32 = 9;
pub const ROOMSLIST: u32 = 10;
pub const EVENT: u32 = 11;
pub const SWITCH: u32 = 12;
pub const LEAVE: u32 = 13;
pub const DISCONNECT: u32 = 14;
pub const ACCEPTED: u32 = 15;
