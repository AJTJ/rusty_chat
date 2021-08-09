// UTILS
use chrono::prelude::*;

// SERDE
use serde::{Deserialize, Serialize};

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
