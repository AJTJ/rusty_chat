// UTILS
use chrono::prelude::*;

// ACTIX
use actix::prelude::*;

// SERDE
use serde::{Deserialize, Serialize};

// MODS
use crate::socket_actor::WebSocketActor;

#[derive(Serialize, Deserialize, Debug)]
pub struct DatabaseMessage {
    pub id: i64,
    pub user_id: i64,
    pub room_id: i64,
    pub message: String,
    pub time: NaiveDateTime,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MessageToDatabase {
    pub user_id: i64,
    pub room_id: i64,
    pub message: String,
    #[serde(default = "default_time")]
    pub time: NaiveDateTime,
}

fn default_time() -> NaiveDateTime {
    Utc::now().naive_utc()
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FromClient {
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ResponseToClient {
    pub user_name: String,
    pub all_messages: String,
    pub message_to_client: String,
    pub is_update: bool,
    pub all_online_users: Vec<String>,
}

// SOCKET THINGS
pub type UniversalIdType = [u8; 32];
// #[derive(Debug)]
pub struct OpenSocketData {
    pub addr: Addr<WebSocketActor>,
    pub user_name: String,
}

// NOTE: SESSION ID AND COOKIE ID ARE THE SAME THING

// SESSION THINGS
pub type SessionID = String;

#[derive(Debug)]
pub struct SessionData {
    pub user_name: String,
    pub expiry: NaiveDateTime,
}

// SHARED COOKIE THINGS
pub const COOKIE_NAME: &str = "rusty_cookie";
#[derive(Serialize, Deserialize, Debug)]
pub struct CookieStruct {
    pub id: String,
    pub user_name: String,
}
