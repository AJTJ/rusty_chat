use actix::prelude::*;
use actix::{Actor, ActorFuture, ContextFutureSpawner, Running, StreamHandler, WrapFuture};
use actix_cors::Cors;
use actix_files as fs;
use actix_web::HttpMessage;
use actix_web::{
    cookie, http, middleware::Logger, web, App, Error, HttpRequest, HttpResponse, HttpServer,
    Result,
};
use actix_web_actors::ws::{self, WebsocketContext};
use argon2::{self, Config};
use chrono::{prelude::*, Duration};
use dotenv::dotenv;
use hotwatch::{Event, Hotwatch};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::collections::HashMap;

use std::sync::Mutex;
use std::time::{Duration as StdDuration, Instant};
use time::{Duration as TimeDuration, OffsetDateTime};

// CONST DATA
// SERVER SEND TIME
const HEARTBEAT_INTERVAL: StdDuration = StdDuration::from_secs(5);
// DEADLINE
const CLIENT_TIMEOUT: StdDuration = StdDuration::from_secs(10);
// COOKIE NAME
const COOKIE_NAME: &str = "rusty_cookie";

// COOKIE
#[derive(Serialize, Deserialize, Debug)]
struct CookieStruct {
    id: String,
    user_name: String,
}

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
    all_online_users: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Room {
    id: i64,
    name: String,
}

// SESSION AND AUTH THINGS
#[derive(Debug)]
struct SessionData {
    user_name: String,
    expiry: NaiveDateTime,
}

#[derive(Debug)]
struct OpenSocketData {
    addr: Addr<WebSocketActor>,
    user_name: String,
}

type SocketId = [u8; 32];
type SessionID = String;

// MESSAGES

// UPDATE ALL SOCKETS
#[derive(Message)]
#[rtype(result = "Result<bool, std::io::Error>")]
struct Resend;

// WS ACTOR
#[derive(Debug)]
struct WebSocketActor {
    db_pool: web::Data<SqlitePool>,
    open_sockets_data: web::Data<Mutex<HashMap<SocketId, OpenSocketData>>>,
    hb: Instant,
    session_table_data: web::Data<Mutex<HashMap<SessionID, SessionData>>>,
    signed_in_user: String,
    session_id: String,
    socket_id: [u8; 32],
}

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

// Handler for Resend Message
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
        message_to_client: "Welcome!".to_string(),
        is_update,
        all_online_users: all_socket_users,
    };
    json!(response).to_string()
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
        message_to_client: "Awesome".to_string(),
        is_update: false,
        all_online_users: all_socket_users,
    };
    resend_ws(open_sockets_data).await;

    json!(response_struct).to_string()
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

// WEBSOCKET INDEX HANDLING
async fn index(
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

// AUTH HANDLING
#[derive(Serialize, Deserialize, Debug)]
struct SignInSignUp {
    user_name: String,
    password: String,
}

// SIGN UP
async fn signup(
    req_body: String,
    db_pool: web::Data<SqlitePool>,
    session_table_data: web::Data<Mutex<HashMap<SessionID, SessionData>>>,
    req: HttpRequest,
) -> HttpResponse {
    // I/O DATA
    let body_json: SignInSignUp =
        serde_json::from_str(&req_body).expect("Error in Client msg to sign in");
    let user_name = &body_json.user_name;
    let password = body_json.password;

    if user_name.is_empty() || password.is_empty() {
        return HttpResponse::Ok().body(json!(format!("Please type a name/password")));
    }

    // CHECK FOR USER IN DB
    let user = sqlx::query!(r#"SELECT id FROM user WHERE name=$1"#, user_name)
        .fetch_one(db_pool.get_ref())
        .await;

    match user {
        // USER EXISTS -> EXIT
        Ok(_) => HttpResponse::Ok().body(json!(format!("User already exists"))),
        // NO USER -> SIGNUP & SIGNIN
        Err(_) => {
            // SAVE THE USER_NAME, PW, SALT
            let config = Config::default();
            let salt_gen = rand::thread_rng().gen::<[u8; 16]>();
            let salt: &[u8] = &salt_gen[..];
            let password_hash = argon2::hash_encoded(password.as_bytes(), &salt, &config).unwrap();
            sqlx::query!(
                r#"INSERT INTO user (name, hash, salt) VALUES ($1, $2, $3)"#,
                user_name,
                password_hash,
                salt
            )
            .execute(db_pool.get_ref())
            .await
            .expect("Saving new user did NOT work");

            // LOGIN
            login_process(session_table_data, req, user_name)
        }
    }
}

fn login_process(
    session_table_data: web::Data<Mutex<HashMap<SessionID, SessionData>>>,
    req: HttpRequest,
    user_name: &String,
) -> HttpResponse {
    let session_table_ref = session_table_data.get_ref();
    let mut session_table = session_table_ref.lock().unwrap();

    // CREATE SESSION ID
    let id = rand::thread_rng().gen::<[u8; 16]>();
    let encoded_session_id = base64::encode(id);

    // BASIC CLEANUP
    session_table.retain(|_, session| session.expiry > Utc::now().naive_utc());

    // CHECK IF SIGNED IN ELSEWHERE/ALREADY
    if session_table.values().any(|x| &x.user_name == user_name) {
        return HttpResponse::Ok().body(json!(format!("{} is already signed in", user_name)));
    };

    // IF A COOKIE IS PRESENT - REMOVE IT FROM SESSION
    let cookie_option = req.cookie(COOKIE_NAME);
    match cookie_option {
        Some(cookie) => {
            let (_, value) = cookie.name_value();
            let cookie_data: CookieStruct =
                serde_json::from_str(value).expect("parsing cookie error");
            session_table.remove_entry(&cookie_data.id);
        }
        None => {}
    }

    // ADD TO SESSION TABLE
    let current_session_data = SessionData {
        user_name: user_name.clone(),
        expiry: Utc::now().naive_utc() + Duration::minutes(5),
    };
    session_table.insert(encoded_session_id.clone(), current_session_data);

    // CREATE NEW COOKIE
    let cookie_values = CookieStruct {
        id: encoded_session_id,
        user_name: user_name.clone(),
    };
    let cookie = cookie::Cookie::build(COOKIE_NAME, json!(cookie_values).to_string())
        .path("/")
        .secure(false)
        .max_age(TimeDuration::minutes(30))
        .http_only(true)
        .finish();

    // RETURN AND ADD/REWRITE COOKIE
    HttpResponse::Ok()
        .cookie(cookie)
        .body(json!(format!("Welcome {}", user_name)))
}

// LOGIN
async fn login(
    req_body: String,
    db_pool: web::Data<SqlitePool>,
    session_table_data: web::Data<Mutex<HashMap<SessionID, SessionData>>>,
    req: HttpRequest,
) -> HttpResponse {
    // INCOMING DATA
    let body_json: SignInSignUp = serde_json::from_str(&req_body).expect("error in login body");
    let user_name = &body_json.user_name;
    let password = &body_json.password;

    // CHECK USER NAME
    match sqlx::query!(r#"SELECT * FROM user WHERE name=$1"#, user_name)
        .fetch_one(db_pool.get_ref())
        .await
    {
        Ok(user_record) => {
            // CHECK PASSWORD
            let pw_hash = user_record.hash;
            let password_match = argon2::verify_encoded(&pw_hash, password.as_bytes()).unwrap();
            match password_match {
                // CORRECT PW AND LOGIN
                true => login_process(session_table_data, req, user_name),
                // WRONG PASSWORD
                false => HttpResponse::Ok().body(json!(format!(
                    "Password does not match the user: {}",
                    user_name
                ))),
            }
        }
        // USER NOT IN DB
        Err(_) => HttpResponse::Ok().body(json!(format!("User name is not registered."))),
    }
}

async fn logout(
    req: HttpRequest,
    session_table_data: web::Data<Mutex<HashMap<SessionID, SessionData>>>,
) -> HttpResponse {
    let cookie_option = req.cookie(COOKIE_NAME);
    match cookie_option {
        Some(cookie) => {
            // REMOVE FROM SESSION
            let session_table_ref = session_table_data.get_ref();
            let mut session_table = session_table_ref.lock().unwrap();
            let (_, value) = cookie.name_value();
            let cookie_data: CookieStruct =
                serde_json::from_str(value).expect("parsing cookie error");
            session_table.remove_entry(&cookie_data.id);

            // TODO REMOVE SOCKET FROM ACTIVE SOCKETS

            // REMOVE COOKIE BY REPLACING WITH ALREADY EXPIRED COOKIE
            let cookie = cookie::Cookie::build(COOKIE_NAME, "should_be_expired".to_string())
                .path("/")
                .secure(false)
                .expires(OffsetDateTime::unix_epoch())
                .http_only(true)
                .finish();

            HttpResponse::Ok().cookie(cookie).finish()
        }
        None => HttpResponse::Ok().finish(),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // ENV
    dotenv().ok();

    // HOT RELOADING
    let mut hotwatch = Hotwatch::new().expect("hotwatch failed to initialize!");
    hotwatch
        .watch("./chat_socket/build", |event: Event| {
            if let Event::Write(_path) = event {
                println!("Changes in front-end");
            }
        })
        .expect("failed to watch file!");

    // OPEN SOCKETS DATA
    let socket_data_hashmap: HashMap<SocketId, OpenSocketData> = HashMap::new();
    let open_sockets_data = web::Data::new(Mutex::new(socket_data_hashmap));

    // SESSION TABLE
    let session_table_hashmap: HashMap<SessionID, SessionData> = HashMap::new();
    let session_table_data = web::Data::new(Mutex::new(session_table_hashmap));

    // DB POOL
    let db_pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite://./chat.db")
        .await
        .expect("pool FAILURE");
    let shared_db_pool = web::Data::new(db_pool);

    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin("http://localhost:3000/")
            .supports_credentials()
            .allowed_origin_fn(|origin, _req_head| origin.as_bytes().ends_with(b".rust-lang.org"))
            .allowed_methods(vec!["GET", "POST"])
            .allowed_headers(vec![http::header::AUTHORIZATION, http::header::ACCEPT])
            // .allowed_header(http::header::CONTENT_TYPE)
            .max_age(3600);
        App::new()
            .wrap(cors)
            .wrap(Logger::default())
            // DATA
            .app_data(shared_db_pool.clone())
            .app_data(open_sockets_data.clone())
            .app_data(session_table_data.clone())
            // ENDPOINTS
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
