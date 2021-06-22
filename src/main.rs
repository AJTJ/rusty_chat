use actix::prelude::*;
use actix::{Actor, ActorFuture, ContextFutureSpawner, Running, StreamHandler, WrapFuture};
use actix_files as fs;
use actix_identity::{CookieIdentityPolicy, Identity, IdentityService, RequestIdentity};
use actix_session::{CookieSession, Session};
use actix_web::{
    middleware::Logger, web, App, Error, HttpRequest, HttpResponse, HttpServer, Result,
};
use actix_web_actors::ws::{self, WebsocketContext};
use argon2::{self, Config};
use chrono;
use chrono::prelude::*;
use dotenv::dotenv;
use hotwatch::{Event, Hotwatch};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::collections::HashMap;
// use std::fmt::{Debug, Formatter, Result};
use std::fmt;
use std::sync::Mutex;
use std::time::{Duration, Instant};

// UNNEEDED CURRENTLY
// use actix_cors::Cors;
// use std::path::PathBuf;
// use fs::NamedFile;
// use sqlx::prelude;

// Salt for argon2
const SALT: &[u8] = b"randomsaltyness";
/// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

type ServerID = [u8; 32];

// ALL STRUCTS

#[derive(Serialize, Deserialize, Debug)]
struct User {
    id: i64,
    name: String,
    password: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct DatabaseMessage {
    id: i64,
    user_id: i64,
    room_id: i64,
    message: String,
    time: NaiveDateTime,
    name: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct MessageToDatabase {
    user_id: i64,
    room_id: i64,
    message: String,
    #[serde(default = "default_time")]
    time: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug)]
struct FromClient {
    message: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct SentToClient {
    user_name: String,
    room_id: i64,
    message: String,
    #[serde(default = "default_time")]
    time: NaiveDateTime,
}

fn default_time() -> NaiveDateTime {
    Utc::now().naive_utc()
}

#[derive(Serialize, Deserialize, Debug)]
struct ResponseToClient {
    user_name: String,
    all_messages: String,
    message_to_client: String,
    is_update: bool,
    all_users: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Room {
    id: i64,
    name: String,
}

// SESSION TYPES

type SessionID = String;
const SESSION_ID_KEY: &str = "id";

// MESSAGES

/// Define message for resetting the ws
#[derive(Message)]
#[rtype(result = "Result<bool, std::io::Error>")]
struct ResetMessage;

/// Msg to signal a resend of information
// #[derive(Message)]
// #[rtype(result = "Result<bool, std::io::Error>")]
// struct ResendMessage;

/// Msg to resend
#[derive(Message)]
#[rtype(result = "Result<bool, std::io::Error>")]
struct Resend;

struct DebugSession(pub Session);

impl fmt::Debug for DebugSession {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

#[derive(Message)]
#[rtype(result = "Result<bool, std::io::Error>")]
struct Ping;
#[derive(Message)]
#[rtype(result = "Result<bool, std::io::Error>")]
struct GetIdentity;

// ID ACTOR

struct IDActor {
    session: Session,
}

impl Actor for IDActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        println!("id actor started");
    }

    fn stopped(&mut self, ctx: &mut Self::Context) {
        println!("id actor stopped");
    }
}

impl Handler<GetIdentity> for IDActor {
    type Result = Result<bool, std::io::Error>;

    fn handle(&mut self, _: GetIdentity, _: &mut Context<Self>) -> Self::Result {
        // println!("Get ID msg received in idactor");
        // if let Some(id) = self.id.identity() {
        //     println!("id in idactor: {}", id);
        // } else {
        //     println!("no id in idactor")
        // }

        let sesh: Result<Option<SessionID>> = self.session.get(SESSION_ID_KEY);
        // let signed_in_user: Option<String>;

        match sesh {
            Ok(s) => {
                if let Some(id) = s {
                    println!("Session id idactor: {}", id);
                } else {
                    println!("Result but no id idactor");
                }
            }
            Err(_) => {
                println!("no result idactor");
            }
        };

        Ok(true)
    }
}

// WS ACTOR
// #[derive(Debug)]
struct WebSocketActor {
    // id: [u8; 32],
    all_messages: serde_json::Value,
    db_pool: web::Data<SqlitePool>,
    // signed_in_user: Option<String>,
    all_users: web::Data<Mutex<Vec<String>>>,
    all_socket_addresses: web::Data<Mutex<HashMap<[u8; 32], Addr<WebSocketActor>>>>,
    // private_key: web::Data<[u8; 32]>,
    // private_key: web::Data<Mutex<Vec<[u8; 32]>>>,
    hb: Instant,
    req: HttpRequest,
    // session: Session,
    id_addr: Addr<IDActor>,
}

impl Actor for WebSocketActor {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        println!("WS Actor STARTED");
        // Start the heartbeats
        // self.hb(ctx);

        // get all_sockets
        let all_addresses_ref = self.all_socket_addresses.get_ref();
        let mut all_sockets = all_addresses_ref.lock().unwrap();

        // generate new key
        let generated_key = rand::thread_rng().gen::<[u8; 32]>();

        // get address
        let addr = ctx.address();

        // insert new key and address into all_sockets of addresses
        all_sockets.insert(generated_key, addr);

        // get where the local ws key is saved
        // let key_ref = self.private_key.get_ref();
        // let mut key_vec = key_ref.lock().unwrap();

        // save local key
        // key_vec.push(generated_key);
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        println!("WS Actor STOPPING");
        Running::Stop
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        println!("WS Actor STOPPED");
        // Remove user from active users if websocket actor closes
        // remove_user_from_users(self.all_users.clone(), self.signed_in_user.clone());

        // get all_sockets and local private key
        let all_addresses_ref = self.all_socket_addresses.get_ref();
        // let key_ref = self.private_key.get_ref();
        // let key = key_ref.lock().unwrap();
        // match Ok(key[0]) {
        //     Ok(k) =>
        // }
        // all_addresses_ref.lock().unwrap().retain(|&k, _| k != key);
        // println!("POST all_sockets ref: {:?}", all_addresses_ref.lock().unwrap());
    }
}

// Handler for Reset Message
impl Handler<ResetMessage> for WebSocketActor {
    type Result = Result<bool, std::io::Error>;

    fn handle(&mut self, _: ResetMessage, ctx: &mut WebsocketContext<Self>) -> Self::Result {
        self::WebsocketContext::stop(ctx);
        Ok(true)
    }
}

// Handler for Resend Message
impl Handler<Resend> for WebSocketActor {
    type Result = Result<bool, std::io::Error>;

    fn handle(&mut self, _: Resend, ctx: &mut WebsocketContext<Self>) -> Self::Result {
        println!("In Resend handle");
        self.update_ws(ctx);
        Ok(true)
    }
}

impl WebSocketActor {
    fn update_ws(&mut self, ctx: &mut WebsocketContext<Self>) {
        update_in_stream(self, ctx, true);
    }

    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // check client heartbeats
            println!("interval started");
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                // heartbeat timed out
                println!("Websocket Client heartbeat failed, disconnecting!");

                // notify chat server
                // act.addr.do_send(server::Disconnect { id: act.id });

                // stop actor
                self::WebsocketContext::stop(ctx);

                // don't try to send a ping
                return;
            }

            ctx.ping(b"");
        });
    }
}

fn update_in_stream(
    the_self: &mut WebSocketActor,
    ctx: &mut WebsocketContext<WebSocketActor>,
    is_update: bool,
) {
    // let msg = match the_self.signed_in_user.clone() {
    //     Some(u) => format!("Welcome {}!", u),
    //     None => "Welcome!".to_string(),
    // };

    let users_struct = the_self.all_users.get_ref().lock().unwrap();
    let users_struct_clone = users_struct.clone();

    let response = ResponseToClient {
        // user_name: the_self.signed_in_user.clone().unwrap_or("".to_string()),
        user_name: "Biff".to_string(),
        all_messages: the_self.all_messages.to_string(),
        // message_to_client: msg,
        message_to_client: "".to_string(),
        is_update,
        all_users: users_struct_clone,
    };
    ctx.text(json!(response).to_string());
}

// WS STREAM HANDLER
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WebSocketActor {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        // let sesh: Result<Option<SessionID>> = self.session.get(SESSION_ID_KEY);
        // let signed_in_user: Option<String>;

        // match sesh {
        //     Ok(s) => {
        //         if let Some(id) = s {
        //             // println!("Session id in stream: {}", id);
        //             signed_in_user = Some(id);
        //         } else {
        //             // println!("Result but no id in stream");
        //             signed_in_user = None;
        //         }
        //     }
        //     Err(_) => {
        //         println!("no result in stream");
        //         signed_in_user = None;
        //     }
        // }

        println!("testing idactor in streamhandler");
        let res = self
            .id_addr
            .send(GetIdentity)
            .into_actor(self)
            .map(|_, _, _| println!("in thing"))
            .wait(ctx);

        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg)
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
            Ok(ws::Message::Text(json_message)) => message_handler(
                json_message,
                self.db_pool.clone(),
                // signed_in_user.clone(),
                // Some("Timmy".to_string()),
                self.all_users.clone(),
                // self.req.clone(),
                self.id_addr.clone(),
            )
            .into_actor(self)
            .map(|text, _, inner_ctx| inner_ctx.text(text))
            .wait(ctx),
            Ok(ws::Message::Binary(_)) => println!("Unexpected binary"),
            _ => (),
        };

        // update other chats
        // resend_ws(self.all_socket_addresses.clone(), self.private_key.clone())
        // .into_actor(self)
        // .map(|_, _, _| println!("in resend map"))
        // .spawn(ctx);
    }

    fn started(&mut self, ctx: &mut Self::Context) {
        // println!("StreamHandler STARTED");
        update_in_stream(self, ctx, false);
    }

    fn finished(&mut self, _: &mut Self::Context) {
        // remove_user_from_users(self.all_users.clone(), self.signed_in_user.clone());
        // println!("StreamHandler FINISHED")
    }
}

// WEBSOCKET MESSAGE HANDLING
async fn message_handler(
    received_client_message: String,
    db_pool: web::Data<SqlitePool>,
    // signed_in_user: Option<String>,
    all_users: web::Data<Mutex<Vec<String>>>,
    id_addr: Addr<IDActor>,
    // req: HttpRequest,
) -> String {
    let from_client: FromClient = serde_json::from_str(&received_client_message)
        .expect("parsing received_client_message msg");

    // let cur_id = req.get_identity();

    // match cur_id {
    //     Some(id) => println!("cur_id {}", id),
    //     None => println!("no cur id"),
    // }

    println!("testing idactor");
    let res = id_addr.send(GetIdentity).await;

    let users_struct = all_users.get_ref().lock().unwrap();
    let users_struct_clone = users_struct.clone();

    let default_room = "lobby".to_string();

    // let msg = match &signed_in_user {
    //     Some(u) => format!("Welcome {}!", u),
    //     None => "Welcome!".to_string(),
    // };

    // match signed_in_user {
    //     Some(id) => {
    //         // ADD TO MESSAGES
    //         let user_id_record = sqlx::query!(r#"SELECT id FROM user WHERE name=$1"#, id)
    //             .fetch_one(db_pool.get_ref())
    //             .await
    //             .expect("user_id_record not found");

    //         let room_id_record = sqlx::query!(r#"SELECT id FROM room WHERE name=$1"#, default_room)
    //             .fetch_one(db_pool.get_ref())
    //             .await
    //             .expect("room_id_record not found");

    //         let message_to_db = MessageToDatabase {
    //             user_id: user_id_record.id,
    //             room_id: room_id_record.id,
    //             message: from_client.message,
    //             time: Utc::now().naive_utc(),
    //         };

    //         let current_time = Utc::now().naive_utc();

    //         sqlx::query!(
    //             r#"INSERT INTO message (user_id, room_id, message, time) VALUES ($1, $2, $3, $4)"#,
    //             message_to_db.user_id,
    //             message_to_db.room_id,
    //             message_to_db.message,
    //             current_time
    //         )
    //         .execute(db_pool.get_ref())
    //         .await
    //         .expect("query insert message error");

    //         let all_messages_json = get_all_messages_json(db_pool.clone()).await;

    //         let response_struct = ResponseToClient {
    //             user_name: id,
    //             all_messages: all_messages_json.to_string(),
    //             message_to_client: msg,
    //             all_users: users_struct_clone,
    //             is_update: false,
    //         };

    //         json!(response_struct).to_string()
    //     }
    //     None => {
    //         let all_messages_json = get_all_messages_json(db_pool.clone()).await;

    //         let mut response_struct = ResponseToClient {
    //             user_name: "".to_string(),
    //             all_messages: all_messages_json.to_string(),
    //             message_to_client: msg,
    //             all_users: users_struct_clone,
    //             is_update: false,
    //         };

    //         response_struct.message_to_client = "Not Signed In".to_string();
    //         json!(response_struct).to_string()
    //     }
    // }

    let users_struct = all_users.get_ref().lock().unwrap();
    let users_struct_clone = users_struct.clone();
    let all_messages_json = get_all_messages_json(db_pool.clone()).await;

    let response = ResponseToClient {
        // user_name: the_self.signed_in_user.clone().unwrap_or("".to_string()),
        user_name: "Biff".to_string(),
        all_messages: all_messages_json.to_string(),
        // message_to_client: msg,
        message_to_client: "".to_string(),
        is_update: false,
        all_users: users_struct_clone,
    };
    json!(response).to_string()
}

// HELPER FUNCTIONS
async fn get_all_messages_json(db_pool: web::Data<SqlitePool>) -> Value {
    let all_messages: Vec<DatabaseMessage> =
        sqlx::query_as!(DatabaseMessage, "SELECT message.id, user_id, room_id, message, time, name FROM message INNER JOIN user on user.id=message.user_id ORDER BY time DESC")
            .fetch_all(db_pool.get_ref())
            .await
            .expect("all messages failure");
    let all_messages_json = json!(&all_messages);
    all_messages_json
}

async fn reset_ws(
    all_socket_addresses: web::Data<Mutex<HashMap<[u8; 32], Addr<WebSocketActor>>>>,
    // private_key: web::Data<Mutex<Vec<[u8; 32]>>>,
) {
    let ws_hm = all_socket_addresses.get_ref().lock().unwrap();
    // println!("hashmap: {:?}", ws_hm);
    // let ws_key = private_key.get_ref();
    // let key = ws_key.lock().unwrap()[0];
    // println!("private key: {:?}", ws_key);
    // // let ws_add = ws_hm.get(&key);
    // match ws_add {
    //     Some(add) => {
    //         let result = add.send(ResetMessage).await;
    //         match result {
    //             Ok(res) => println!("Got reset result: {}", res.unwrap()),
    //             Err(err) => println!("Got reset error: {}", err),
    //         }
    //     }
    //     None => println!("no ws add"),
    // }
}

async fn resend_ws(
    all_socket_addresses: web::Data<Mutex<HashMap<[u8; 32], Addr<WebSocketActor>>>>,
    // private_key: web::Data<Mutex<Vec<[u8; 32]>>>,
) {
    let mut addresses_hashmap = all_socket_addresses.get_ref().lock().unwrap();
    // let key_ref = private_key.get_ref();
    // let key = key_ref.lock().unwrap()[0];
    // all_addresses_ref.lock().unwrap().retain(|&k, _| k != *key_ref);
    // addresses_hashmap.retain(|&k, _| k != key);
    // println!("ORIGINAL all_sockets: {:?}", all_socket_addresses.lock().unwrap());
    // println!("NEW all_sockets: {:?}", addresses_hashmap);
    for (_, add) in addresses_hashmap.iter() {
        let result = add.send(Resend).await;
        match result {
            Ok(res) => println!("Got resend result: {}", res.unwrap()),
            Err(err) => println!("Got resend error: {}", err),
        }
    }
}

fn remove_user_from_users(all_users: web::Data<Mutex<Vec<String>>>, id: Option<String>) {
    if let Some(current_id) = id {
        // println!("current_id: {}", current_id);
        let mut users = all_users.get_ref().lock().unwrap();
        let usr_idx = users.iter().position(|x| x == &current_id);

        match usr_idx {
            Some(idx) => {
                println!(
                    "removing user usr idx: {}, current_id: {}",
                    idx, &current_id
                );
                users.remove(idx);
            }
            None => {}
        }
    };
}

// WEBSOCKET INDEX HANDLING
async fn index(
    db_pool: web::Data<SqlitePool>,
    req: HttpRequest,
    stream: web::Payload,
    id: Identity,
    all_socket_addresses: web::Data<Mutex<HashMap<[u8; 32], Addr<WebSocketActor>>>>,
    all_users: web::Data<Mutex<Vec<String>>>,
    session: Session,
    // private_key: web::Data<Mutex<Vec<[u8; 32]>>>,
) -> Result<HttpResponse, Error> {
    let all_messages = get_all_messages_json(db_pool.clone()).await;

    // if let Some(id) = id.identity() {
    //     println!("id in index: {}", id);
    // } else {
    //     println!("no id in index")
    // }

    // let sesh: Result<Option<SessionID>> = session.get(SESSION_ID_KEY);
    // // let signed_in_user: Option<String>;

    // match sesh {
    //     Ok(s) => {
    //         if let Some(id) = s {
    //             println!("Session id index: {}", id);
    //         } else {
    //             println!("Result but no id index");
    //         }
    //     }
    //     Err(_) => {
    //         println!("no result index");
    //     }
    // }

    let id_addr = IDActor { session }.start();

    let res = id_addr.send(GetIdentity).await;

    // let signed_in_user = id.identity();
    // let cur_id = req.get_identity();

    // match cur_id {
    //     Some(id) => println!("cur_id index {}", id),
    //     None => println!("no cur id index"),
    // }

    // let key_ref = private_key.get_ref();
    // let key = key_ref.lock().unwrap()[0];

    // println!("privatekey: {:?}", key);

    // add user to all users if they are signed in and not already in the list of all users
    // match &signed_in_user {
    //     Some(user) => {
    //         let mut users = all_users.get_ref().lock().unwrap();
    //         let usr_idx = users.iter().position(|x| x == user);
    //         match usr_idx {
    //             Some(_) => {}
    //             None => {
    //                 println!("adding user {}", user);
    //                 users.push(user.clone())
    //             }
    //         }
    //     }
    //     None => {}
    // }

    // println!("The ext: {:?}", ext);

    let generated_key = rand::thread_rng().gen::<[u8; 32]>();
    // println!("the key {:?}", generated_key);

    // let new_id: ServerID = generated_key;
    // req.extensions_mut().insert(new_id);

    let response = ws::start(
        WebSocketActor {
            // id: generated_key,
            all_messages,
            db_pool,
            all_users: all_users.clone(),
            all_socket_addresses: all_socket_addresses.clone(),
            hb: Instant::now(),
            req: req.clone(),
            // session,
            id_addr,
        },
        &req,
        stream,
    );

    response
}

// AUTH HANDLING
#[derive(Serialize, Deserialize, Debug)]
struct SignInSignUp {
    user_name: String,
    password: String,
}

// SIGN UP
async fn signup(
    // id: Identity,
    req_body: String,
    db_pool: web::Data<SqlitePool>,
    all_socket_addresses: web::Data<Mutex<HashMap<[u8; 32], Addr<WebSocketActor>>>>,
    all_users: web::Data<Mutex<Vec<String>>>,
    // private_key: web::Data<Mutex<Vec<[u8; 32]>>>,
) -> HttpResponse {
    // println!("Signup request body: {:?}", req_body);
    let body_json: SignInSignUp =
        serde_json::from_str(&req_body).expect("Error in Client msg to sign in");
    let user_name = &body_json.user_name;
    let password = body_json.password;

    // let current_user = id.identity();
    // match current_user {
    //     Some(_) => {
    //         HttpResponse::Ok().body(json!(format!("Currently signed in")));
    //     }
    //     None => {}
    // }

    // check if user name exists
    let user = sqlx::query!(r#"SELECT id FROM user WHERE name=$1"#, user_name)
        .fetch_one(db_pool.get_ref())
        .await;

    match user {
        // If user name exists, exit the process
        Ok(_) => HttpResponse::Ok().body(json!(format!("User already exists"))),
        // if user does NOT exist, then sign them up
        Err(_) => {
            // going to sign up/sign in, so remove previous user from all users if already there.
            // remove_user_from_users(all_users.clone(), id.identity());

            let config = Config::default();
            let password_hash = argon2::hash_encoded(password.as_bytes(), SALT, &config).unwrap();

            // SAVE THE USER
            sqlx::query!(
                r#"INSERT INTO user (name, password) VALUES ($1, $2)"#,
                user_name,
                password_hash
            )
            .execute(db_pool.get_ref())
            .await
            .expect("Saving new user did NOT work");

            // save the user for this session
            // id.remember(body_json.user_name.to_owned());

            // add to all users
            let mut users = all_users.get_ref().lock().unwrap();
            users.push(user_name.clone());

            // reset ws
            // reset_ws(all_socket_addresses, private_key).await;
            HttpResponse::Ok().body(json!(format!("New user added")))
        }
    }
}
/**
    LOGIN
    Graceful error if receiving wrong data from frontend.
*/
async fn login(
    req_body: String,
    db_pool: web::Data<SqlitePool>,
    all_socket_addresses: web::Data<Mutex<HashMap<[u8; 32], Addr<WebSocketActor>>>>,
    all_users: web::Data<Mutex<Vec<String>>>,
    req: HttpRequest, // private_key: web::Data<Mutex<Vec<[u8; 32]>>>,
    id: Identity,
    session: Session,
) -> HttpResponse {
    let body_json: SignInSignUp = serde_json::from_str(&req_body).expect("error in login body");
    let user_name = &body_json.user_name;
    let password = &body_json.password;

    // let ext = req.extensions();
    // let server_id_option = ext.get::<ServerID>();

    // match server_id_option {
    //     Some(id) => println!("id is {:?}", id),
    //     None => println!("nothing in server_id_option"),
    // }

    // Check for user name
    match sqlx::query!(r#"SELECT * FROM user WHERE name=$1"#, user_name)
        .fetch_one(db_pool.get_ref())
        .await
    {
        Ok(user_record) => {
            let pw_hash = user_record.password;
            let password_match = argon2::verify_encoded(&pw_hash, password.as_bytes()).unwrap();
            let mut msg_to_client = "You are logged in as: ";

            //check if password matches
            match password_match {
                true => {
                    // remove previous user if already logged in
                    // let current_id = id.identity();
                    // remove_user_from_users(all_users.clone(), current_id);

                    // Remember the new user
                    let current_id: SessionID = user_name.to_string();

                    id.remember(current_id.to_owned());
                    if let Some(id) = id.identity() {
                        println!("the id is: {}", id);
                    }

                    let sesh = session.set(SESSION_ID_KEY, &current_id);

                    match sesh {
                        Ok(_) => println!("session set"),
                        Err(e) => println!("session not set: {}", e),
                    }

                    // add to all users
                    // let mut users = all_users.get_ref().lock().unwrap();
                    // users.push(user_name.clone());
                    // Reset the WS
                    // reset_ws(all_socket_addresses, private_key).await;
                }
                false => msg_to_client = "Password does not match this user: ",
            }
            HttpResponse::Ok().body(json!(format!("{} {}", msg_to_client, user_name)))
        }
        Err(_) => HttpResponse::Ok().body(json!(format!("User name does not exist."))),
    }
}

async fn logout(
    // id: Identity,
    all_socket_addresses: web::Data<Mutex<HashMap<[u8; 32], Addr<WebSocketActor>>>>,
    all_users: web::Data<Mutex<Vec<String>>>,
    session: Session, // private_key: web::Data<Mutex<Vec<[u8; 32]>>>,
) -> HttpResponse {
    // if let Some(current_id) = id.identity() {
    //     // remove from all users
    //     remove_user_from_users(all_users, Some(current_id));
    //     // remove auth
    //     id.forget();
    //     // reset ws
    //     // reset_ws(all_socket_addresses, private_key).await;
    //     HttpResponse::Ok().body(json!(format!("Logged Out.")))
    // } else {
    //     HttpResponse::Ok().body(json!(format!("Not logged in.")))
    // }
    session.remove(SESSION_ID_KEY);
    HttpResponse::Ok().finish()
}

// MAIN AND DB INSTANTIATION
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // set env variables for sqlx
    dotenv().ok();

    // Saving all the addresses
    let ws_addr: HashMap<[u8; 32], Addr<WebSocketActor>> = HashMap::new();
    let all_socket_addresses = web::Data::new(Mutex::new(ws_addr));

    // Saving all the online users
    let users_vec: Vec<String> = vec![];
    let all_users = web::Data::new(Mutex::new(users_vec));

    // WATCH FOR FILE CHANGES FOR HOT RELOADING
    let mut hotwatch = Hotwatch::new().expect("hotwatch failed to initialize!");
    hotwatch
        .watch("./chat_socket/build", |event: Event| {
            if let Event::Write(_path) = event {
                println!("Changes in front-end");
            }
        })
        .expect("failed to watch file!");

    // DB POOL
    let db_pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite://./chat.db")
        .await
        .expect("pool FAILURE");

    let shared_db_pool = web::Data::new(db_pool);

    HttpServer::new(move || {
        // generate some sort of private key here?
        // let private_key_hash: Vec<[u8; 32]> = vec![];
        // let private_key = web::Data::new(Mutex::new(private_key_hash));

        App::new()
            .wrap(
                CookieSession::signed(&[0; 32])
                    .name("rusty_chat_actix_session")
                    .secure(false),
            )
            .wrap(IdentityService::new(
                // <- create identity middleware
                CookieIdentityPolicy::new(&[0; 32]) // <- create cookie identity policy
                    .name("rusty_chat_actix_identity")
                    .secure(false),
            ))
            .wrap(Logger::default())
            // The app data
            .app_data(shared_db_pool.clone())
            .app_data(all_socket_addresses.clone())
            .app_data(all_users.clone())
            // .app_data(private_key.clone())
            // the different endpoints
            .route("/signup/", web::post().to(signup))
            .route("/login/", web::post().to(login))
            .route("/logout/", web::get().to(logout))
            .route("/ws/", web::get().to(index))
            .service(fs::Files::new("/", "./chat_socket/build").index_file("./index.html"))
    })
    .bind("127.0.0.1:8081")?
    .run()
    .await
}

// NOTES: about://flags/ samesite flags

// UNUSED IMPORTS
// use actix_web::{http::header, middleware::Logger, App, HttpServer};
// use sqlx::FromRow;

// SAME SITE THINGS NOT USED
// .wrap(
//     // create identity middleware
//     IdentityService::new(
//         // create cookie identity policy
//         CookieIdentityPolicy::new(&[0; 32])
//             .name("auth-cookie")
//             .secure(true)
//             // NOT FOR
//             // .same_site(actix_web::cookie::SameSite::None),
//     ),
// )

// GENERATE A RANDOM 32 BYTE KEY.
// Note that it is important to use a unique
// private key for every project. Anyone with access to the key can generate
// authentication cookies for any user!
// let cookie_key = rand::thread_rng().gen::<[u8; 32]>();

// PW DEMO
// let password = b"password";
// let config = Config::default();
// let all_sockets = argon2::hash_encoded(password, SALT, &config).unwrap();
// let matches = argon2::verify_encoded(&all_sockets, password).unwrap();
// assert!(matches);

// CORS WRAPPING (NO LONGER NEEDED)
// .wrap(
//     Cors::default()
//         // .allowed_origin("http://localhost:3000")
//         .allowed_origin("http://127.0.0.1:8081/")
//         .allowed_methods(vec!["GET", "POST"])
//         .allowed_headers(vec![header::AUTHORIZATION, header::ACCEPT])
//         .allowed_header(header::CONTENT_TYPE)
//         .supports_credentials()
//         .max_age(3600),
// )

// A SINGLE STATIC FILE
// async fn static_files(req: HttpRequest) -> Result<NamedFile> {
//     println!("the req: {:?}", req);
//     let path: PathBuf = req
//         .match_info()
//         .query("./chat_socket/build")
//         .parse()
//         .unwrap();
//     println!("path: {:?}", path);
//     Ok(NamedFile::open(path)?)
// }
