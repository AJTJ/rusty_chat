// ACTIX
use actix::prelude::*;
use actix::{Actor, ActorFuture, ContextFutureSpawner, Running, StreamHandler, WrapFuture};
use actix_web::HttpMessage;
use actix_web::{web, Error, HttpRequest, HttpResponse, Result};
use actix_web_actors::ws::{self, WebsocketContext};

// UTILS
use chrono::{prelude::*, Duration};
use rand::Rng;

// SERDE

use serde_json::{json, Value};
// DB
use sqlx::SqlitePool;

// STD
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

// MODS
use crate::common::{
    CookieStruct, OpenSocketData, SessionData, SessionID, SocketId, CLIENT_TIMEOUT, COOKIE_NAME,
    HEARTBEAT_INTERVAL,
};

use crate::socket_actor_structs::{
    DatabaseMessage, FromClient, MessageToDatabase, ResponseToClient,
};

//

// WS ACTOR DECLARATION
#[derive(Debug)]
pub struct WebSocketActor {
    db_pool: web::Data<SqlitePool>,
    open_sockets_data: web::Data<Mutex<HashMap<SocketId, OpenSocketData>>>,
    hb: Instant,
    session_table_data: web::Data<Mutex<HashMap<SessionID, SessionData>>>,
    signed_in_user: String,
    session_id: String,
    socket_id: [u8; 32],
}

// ACTOR IMPL
impl Actor for WebSocketActor {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // SAVE OPEN SOCKET
        let open_sockets_data_ref = self.open_sockets_data.get_ref();
        let addr = ctx.address();

        let socket_data = OpenSocketData {
            addr,
            user_name: self.signed_in_user.clone(),
        };

        open_sockets_data_ref
            .lock()
            .unwrap()
            .insert(self.socket_id, socket_data);

        // UPDATE OTHER SOCKETS WITH NEW SOCKET PRESENT
        resend_ws(self.open_sockets_data.clone())
            .into_actor(self)
            .map(|_, _, _| {})
            .wait(ctx);

        // START HEARTBEAT
        self.hb(ctx);
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        //REMOVE WS FROM ACTIVE SOCKETS
        let open_sockets_data_ref = self.open_sockets_data.get_ref();
        let mut all_sockets = open_sockets_data_ref.lock().unwrap();
        all_sockets.remove_entry(&self.socket_id);

        Running::Stop
    }

    fn stopped(&mut self, _: &mut Self::Context) {}
}

// RESEND MESSAGE
#[derive(Message)]
#[rtype(result = "Result<bool, std::io::Error>")]
struct Resend;

// RESEND MESSAGE HANDLER FOR UPDATING WS
impl Handler<Resend> for WebSocketActor {
    type Result = Result<bool, std::io::Error>;
    fn handle(&mut self, _: Resend, ctx: &mut WebsocketContext<Self>) -> Self::Result {
        get_update_string(
            self.signed_in_user.to_string(),
            false,
            self.db_pool.clone(),
            self.open_sockets_data.clone(),
        )
        .into_actor(self)
        .map(|text, _, inner_ctx| inner_ctx.text(text))
        .wait(ctx);
        Ok(true)
    }
}

// HEARTBEAT IMPLEMENTATION FOR WS
impl WebSocketActor {
    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // check client heartbeats
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                // heartbeat timed out
                println!("Websocket Client heartbeat failed, disconnecting!");
                self::WebsocketContext::stop(ctx);
                return;
            }
            ctx.ping(b"");
        });
    }
}

// WEBSOCKET INDEX HANDLING
pub async fn ws_index(
    db_pool: web::Data<SqlitePool>,
    req: HttpRequest,
    stream: web::Payload,
    open_sockets_data: web::Data<Mutex<HashMap<SocketId, OpenSocketData>>>,
    session_table_data: web::Data<Mutex<HashMap<SessionID, SessionData>>>,
) -> Result<HttpResponse, Error> {
    let cookie_option = req.cookie(COOKIE_NAME);

    // CHECK IF THERE IS A COOKIE
    match cookie_option {
        Some(cookie) => {
            // CHECK IF SESSION EXISTS
            let session_table_ref = session_table_data.get_ref();
            let mut session_table = session_table_ref.lock().unwrap();

            let (_, value) = cookie.name_value();
            let cookie_data: CookieStruct =
                serde_json::from_str(value).expect("parsing cookie error");

            if let Some(the_sesh_data) = session_table.get_mut(&cookie_data.id) {
                // CHECK IF IT HAS EXPIRED
                let expiry = the_sesh_data.expiry;
                if expiry < Utc::now().naive_utc() {
                    // SESSION HAS ENDED, DELETE FROM TABLE, RETURN RESPONSE
                    session_table.remove_entry(&cookie_data.id);
                    return Ok(HttpResponse::Ok().finish());
                }
                // BOOST EXPIRY DATE IF NOT EXPIRED
                the_sesh_data.expiry = Utc::now().naive_utc() + Duration::minutes(5);
            } else {
                // NO SESSION, NO SOCKET
                return Ok(HttpResponse::Ok().finish());
            }

            // GENERATE SOCKET ID
            let socket_id = rand::thread_rng().gen::<[u8; 32]>();

            // OPEN SOCKET
            let response = ws::start(
                WebSocketActor {
                    db_pool,
                    open_sockets_data: open_sockets_data.clone(),
                    hb: Instant::now(),
                    session_table_data: session_table_data.clone(),
                    signed_in_user: cookie_data.user_name.clone(),
                    session_id: cookie_data.id.clone(),
                    socket_id,
                },
                &req,
                stream,
            );

            response
        }
        // NO COOKIE NO SOCKET
        None => Ok(HttpResponse::Ok().finish()),
    }
}

// WS STREAM HANDLER
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WebSocketActor {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        //CHECK IF SESSION EXISTS ON EVERY INTERACTION
        let session_table_ref = self.session_table_data.get_ref();
        let mut session_table = session_table_ref.lock().unwrap();
        if let Some(the_sesh_data) = session_table.get_mut(&self.session_id) {
            // CHECK IF IT HAS EXPIRED
            let expiry = the_sesh_data.expiry;
            if expiry < Utc::now().naive_utc() {
                // EXPIRED SESSION - REMOVE FROM TABLE - CLOSE SOCKET
                session_table.remove_entry(&self.session_id);
                return ctx.stop();
            }
            // BOOST EXPIRY IF NOT EXPIRED
            the_sesh_data.expiry = Utc::now().naive_utc() + Duration::minutes(5);

            match msg {
                Ok(ws::Message::Ping(_)) => {}
                Ok(ws::Message::Close(_)) => ctx.stop(),
                Ok(ws::Message::Pong(_)) => {
                    self.hb = Instant::now();
                    // UPDATE ON HEARTBEAT
                    get_update_string(
                        self.signed_in_user.to_string(),
                        false,
                        self.db_pool.clone(),
                        self.open_sockets_data.clone(),
                    )
                    .into_actor(self)
                    .map(|text, _, inner_ctx| inner_ctx.text(text))
                    .wait(ctx);
                }
                Ok(ws::Message::Text(json_message)) => message_handler(
                    json_message,
                    self.db_pool.clone(),
                    self.signed_in_user.clone(),
                    self.open_sockets_data.clone(),
                )
                .into_actor(self)
                .map(|text, _, inner_ctx| inner_ctx.text(text))
                .wait(ctx),
                Ok(ws::Message::Binary(_)) => println!("Unexpected binary"),
                _ => (),
            };
            return;
        }
        // NO SESSION - CLOSE SOCKET
        ctx.stop()
    }

    fn started(&mut self, ctx: &mut Self::Context) {
        get_update_string(
            self.signed_in_user.to_string(),
            false,
            self.db_pool.clone(),
            self.open_sockets_data.clone(),
        )
        .into_actor(self)
        .map(|text, _, inner_ctx| inner_ctx.text(text))
        .wait(ctx);
    }

    fn finished(&mut self, _: &mut Self::Context) {}
}

// WEBSOCKET MESSAGE HANDLING
async fn message_handler(
    received_client_message: String,
    db_pool: web::Data<SqlitePool>,
    signed_in_user: String,
    open_sockets_data: web::Data<Mutex<HashMap<SocketId, OpenSocketData>>>,
) -> String {
    let from_client: FromClient = serde_json::from_str(&received_client_message)
        .expect("parsing received_client_message msg");

    let default_room = "lobby".to_string();
    let id = signed_in_user;

    // ADD TO MESSAGES
    let user_id_record = sqlx::query!(r#"SELECT id FROM user WHERE name=$1"#, id)
        .fetch_one(db_pool.get_ref())
        .await
        .expect("user_id_record not found");

    let room_id_record = sqlx::query!(r#"SELECT id FROM room WHERE name=$1"#, default_room)
        .fetch_one(db_pool.get_ref())
        .await
        .expect("room_id_record not found");

    let message_to_db = MessageToDatabase {
        user_id: user_id_record.id,
        room_id: room_id_record.id,
        message: from_client.message,
        time: Utc::now().naive_utc(),
    };

    let current_time = Utc::now().naive_utc();

    sqlx::query!(
        r#"INSERT INTO message (user_id, room_id, message, time) VALUES ($1, $2, $3, $4)"#,
        message_to_db.user_id,
        message_to_db.room_id,
        message_to_db.message,
        current_time
    )
    .execute(db_pool.get_ref())
    .await
    .expect("query insert message error");

    let all_messages_json = get_all_messages_json(db_pool.clone()).await;

    let open_sockets_data_ref = open_sockets_data.get_ref();
    let all_socket_users = open_sockets_data_ref
        .lock()
        .unwrap()
        .iter()
        .map(|(_, add)| add.user_name.clone())
        .collect::<Vec<_>>();

    let response_struct = ResponseToClient {
        user_name: id,
        all_messages: all_messages_json.to_string(),
        message_to_client: "".to_string(),
        is_update: false,
        all_online_users: all_socket_users,
    };
    resend_ws(open_sockets_data).await;

    json!(response_struct).to_string()
}

// UTILITY FUNCTIONS
// GET ALL MSGES FROM DB
async fn get_all_messages_json(db_pool: web::Data<SqlitePool>) -> Value {
    let all_messages: Vec<DatabaseMessage> =
      sqlx::query_as!(DatabaseMessage, "SELECT message.id, user_id, room_id, message, time, name FROM message INNER JOIN user on user.id=message.user_id ORDER BY time DESC LIMIT 50")
          .fetch_all(db_pool.get_ref())
          .await
          .expect("all messages failure");
    let all_messages_json = json!(&all_messages);
    all_messages_json
}

// CREATE UPDATE FOR SOCKET
async fn get_update_string(
    signed_in_user: String,
    is_update: bool,
    db_pool: web::Data<SqlitePool>,
    open_sockets_data: web::Data<Mutex<HashMap<SocketId, OpenSocketData>>>,
) -> String {
    // GET ONLNIE USERS
    let open_sockets_data_ref = open_sockets_data.get_ref();
    let all_socket_users = open_sockets_data_ref
        .lock()
        .unwrap()
        .iter()
        .map(|(_, add)| add.user_name.clone())
        .collect::<Vec<_>>();

    let all_messages = get_all_messages_json(db_pool).await;
    let response = ResponseToClient {
        user_name: signed_in_user,
        all_messages: all_messages.to_string(),
        message_to_client: "".to_string(),
        is_update,
        all_online_users: all_socket_users,
    };
    json!(response).to_string()
}

// GET NEW MESSAGES ON MSG SENT
async fn resend_ws(open_sockets_data: web::Data<Mutex<HashMap<SocketId, OpenSocketData>>>) {
    let open_sockets_data_ref = open_sockets_data.get_ref();
    let all_sockets = open_sockets_data_ref
        .lock()
        .unwrap()
        .iter()
        .map(|(_, add)| add.addr.try_send(Resend))
        .collect::<Vec<_>>();

    for sock in all_sockets {
        // HANGS ON THIS AWAIT
        let result = sock;
        match result {
            Ok(_) => {}
            Err(err) => println!("Got resend error: {:?}", err),
        }
    }
}
