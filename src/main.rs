use actix::prelude::*;
use actix::{Actor, ActorFuture, ContextFutureSpawner, Running, StreamHandler, WrapFuture};
use actix_files as fs;
use actix_identity::{CookieIdentityPolicy, Identity, IdentityService};
use actix_session::Session;
use actix_web::HttpMessage;
use actix_web::{
    cookie, middleware::Logger, web, App, Error, HttpRequest, HttpResponse, HttpServer, Result,
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

use std::fmt;
use std::sync::Mutex;
use std::time::{Duration, Instant};

// Salt for argon2
const SALT: &[u8] = b"randomsaltyness";
/// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

// GENERAL STRUCTS

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

/// Used for resetting a websocket
#[derive(Message)]
#[rtype(result = "Result<bool, std::io::Error>")]
struct ResetMessage;

/// Msg to update all the chat info
#[derive(Message)]
#[rtype(result = "Result<bool, std::io::Error>")]
struct Resend;

struct DebugSession(pub Session);
impl fmt::Debug for DebugSession {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

// Attempting to get identity
#[derive(Message)]
#[rtype(result = "Result<bool, std::io::Error>")]
struct GetIdentity;

// ID ACTOR
struct IDActor {
    session: Session,
}

impl Actor for IDActor {
    type Context = Context<Self>;

    fn started(&mut self, _: &mut Self::Context) {
        println!("id actor started");
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        println!("id actor stopped");
    }
}

impl Handler<GetIdentity> for IDActor {
    type Result = Result<bool, std::io::Error>;

    fn handle(&mut self, _: GetIdentity, _: &mut Context<Self>) -> Self::Result {
        let sesh: Result<Option<SessionID>> = self.session.get(SESSION_ID_KEY);

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
    all_messages: serde_json::Value,
    db_pool: web::Data<SqlitePool>,
    all_users: web::Data<Mutex<Vec<String>>>,
    all_socket_addresses: web::Data<Mutex<HashMap<[u8; 32], Addr<WebSocketActor>>>>,
    hb: Instant,
    req: HttpRequest,
}

impl Actor for WebSocketActor {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        println!("WS Actor STARTED");
        // Start the heartbeats
        self.hb(ctx);

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
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                // heartbeat timed out
                println!("Websocket Client heartbeat failed, disconnecting!");

                // stop actor
                self::WebsocketContext::stop(ctx);

                // don't try to send a ping
                return;
            }
            // println!("heartbeat resetting");
            ctx.ping(b"");
        });
    }
}

fn update_in_stream(
    the_self: &mut WebSocketActor,
    ctx: &mut WebsocketContext<WebSocketActor>,
    is_update: bool,
) {
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
        match msg {
            Ok(ws::Message::Ping(_)) => {}
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
            )
            .into_actor(self)
            .map(|text, _, inner_ctx| inner_ctx.text(text))
            .wait(ctx),
            Ok(ws::Message::Binary(_)) => println!("Unexpected binary"),
            _ => (),
        };
    }

    fn started(&mut self, ctx: &mut Self::Context) {
        update_in_stream(self, ctx, false);
        println!("StreamHandler STARTED");
    }

    fn finished(&mut self, _: &mut Self::Context) {
        println!("StreamHandler FINISHED")
    }
}

// WEBSOCKET MESSAGE HANDLING
async fn message_handler(
    received_client_message: String,
    db_pool: web::Data<SqlitePool>,
    all_users: web::Data<Mutex<Vec<String>>>,
) -> String {
    let from_client: FromClient = serde_json::from_str(&received_client_message)
        .expect("parsing received_client_message msg");

    let default_room = "lobby".to_string();

    // FOR SENDING DEFAULT DATA
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
) -> Result<HttpResponse, Error> {
    let all_messages = get_all_messages_json(db_pool.clone()).await;

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

// fn create_cookie_value() {
//     let id = rand::thread_rng().gen::<[u8; 16]>();
//     let cookie_value = base64::encode(id);
//     let cookie = cookie::Cookie::new("rusty_cookie", cookie_value);
// }

// SIGN UP
async fn signup(
    // id: Identity,
    req_body: String,
    db_pool: web::Data<SqlitePool>,
    all_socket_addresses: web::Data<Mutex<HashMap<[u8; 32], Addr<WebSocketActor>>>>,
    all_users: web::Data<Mutex<Vec<String>>>,
) -> HttpResponse {
    HttpResponse::Ok().body(json!(format!("New user added")))
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
    req: HttpRequest,
    id: Identity,
    session: Session,
) -> HttpResponse {
    let id = rand::thread_rng().gen::<[u8; 16]>();
    let cookie_value = base64::encode(id);
    println!("the id {}", cookie_value);
    let cookie = cookie::Cookie::build("rusty_cookie", cookie_value)
        .path("/")
        .secure(false)
        .http_only(true)
        .finish();
    println!("it be logging in, {}", cookie);
    let res = HttpResponse::Ok().cookie(cookie).finish();
    res
}

async fn logout(
    // id: Identity,
    all_socket_addresses: web::Data<Mutex<HashMap<[u8; 32], Addr<WebSocketActor>>>>,
    all_users: web::Data<Mutex<Vec<String>>>,
    session: Session,
    req: HttpRequest,
) -> HttpResponse {
    let cookies = req.cookies();
    println!("the cookies {:?}", cookies);
    println!("the req {:?}", req);
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
        App::new()
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
