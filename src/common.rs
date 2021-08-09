// ACTIX
use actix::prelude::*;

// UTILS
use chrono::prelude::NaiveDateTime;

// SERDE
use serde::{Deserialize, Serialize};

// STD
use std::time::Duration as StdDuration;

// MODS
use crate::socket_actor::WebSocketActor;

//

// COOKIE NAME
pub const COOKIE_NAME: &str = "rusty_cookie";
// SERVER SEND TIME
pub const HEARTBEAT_INTERVAL: StdDuration = StdDuration::from_secs(5);
// DEADLINE
pub const CLIENT_TIMEOUT: StdDuration = StdDuration::from_secs(10);

// SHARED COOKIE THINGS
#[derive(Serialize, Deserialize, Debug)]
pub struct CookieStruct {
    pub id: String,
    pub user_name: String,
}

// SHARED SOCKET THINGS
#[derive(Debug)]
pub struct SessionData {
    pub user_name: String,
    pub expiry: NaiveDateTime,
}

pub type SessionID = String;
pub type SocketId = [u8; 32];

#[derive(Debug)]
pub struct OpenSocketData {
    pub addr: Addr<WebSocketActor>,
    pub user_name: String,
}
