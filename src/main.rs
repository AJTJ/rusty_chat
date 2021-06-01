use actix::prelude::*;
use actix::{Actor, ActorFuture, ContextFutureSpawner, Running, StreamHandler, WrapFuture};
use actix_cors::Cors;
use actix_files as fs;
use actix_identity::{CookieIdentityPolicy, Identity, IdentityService};
use actix_web::{
    http::header, middleware::Logger, web, App, Either, Error, HttpRequest, HttpResponse,
    HttpServer, Result,
};
use actix_web_actors::ws::{self, WebsocketContext};
use argon2::{self, Config};
use chrono;
use chrono::prelude::*;
use dotenv::dotenv;
use fs::NamedFile;
use hotwatch::{Event, Hotwatch};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::prelude;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::collections::HashMap;
// use actix_web::{HttpRequest, Result};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
mod watcher;
use std::collections::hash_map::RandomState;

const SALT: &[u8] = b"randomsaltyness";

// DATA STRUCTS

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
    #[serde(default = "default_time")]
    time: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug)]
struct FromClient {
    message: String,
    user_name: String,
    password: String,
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
    all_messages: String,
    signed_in: bool,
    #[serde(default = "default_id")]
    id: String,
    // Currently making it a string that says "None" so that the front-end doesn't display it.
    message_to_client: String,
}

fn default_id() -> String {
    "Anonymous".to_string()
}

#[derive(Serialize, Deserialize, Debug)]
struct Room {
    id: i64,
    name: String,
}

// RESET ACTOR INSTANTIATION

/// Define message
#[derive(Message)]
#[rtype(result = "Result<bool, std::io::Error>")]
struct Ping;

/// Define message
#[derive(Message)]
#[rtype(result = "Result<bool, std::io::Error>")]
struct ResetMessage;

// WS ACTOR INSTANTIATION
#[derive(Debug)]
struct WebSocketActor {
    all_messages: serde_json::Value,
    db_pool: web::Data<SqlitePool>,
    current_id: Option<String>,
}

impl Actor for WebSocketActor {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, _: &mut Self::Context) {
        println!("WS Actor STARTED");
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        println!("WS Actor CLOSING");
        Running::Stop
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        println!("WS Actor CLOSED")
    }
}

// Handler for Ping Message
impl Handler<Ping> for WebSocketActor {
    type Result = Result<bool, std::io::Error>;

    fn handle(&mut self, msg: Ping, ctx: &mut WebsocketContext<Self>) -> Self::Result {
        // println!("WS Ping received");
        ctx.stop();
        Ok(true)
    }
}

// Handler for Reset Message
impl Handler<ResetMessage> for WebSocketActor {
    type Result = Result<bool, std::io::Error>;

    fn handle(&mut self, msg: ResetMessage, ctx: &mut WebsocketContext<Self>) -> Self::Result {
        self::WebsocketContext::stop(ctx);
        Ok(true)
    }
}

// WS STREAM HANDLER
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WebSocketActor {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        // let ws_addr = ctx.address();

        if let Some(id) = &self.current_id {
            println!("id in stream handler {}", id)
        } else {
            println!("No id stream handler!")
        }
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(json_message)) => {
                message_handler(json_message, self.db_pool.clone(), self.current_id.clone())
                    .into_actor(self)
                    .map(|text, _, ctx| ctx.text(text))
                    .wait(ctx)
            }
            Ok(ws::Message::Binary(_)) => println!("Unexpected binary"),
            _ => (),
        }
    }

    fn started(&mut self, ctx: &mut Self::Context) {
        println!("StreamHandler STARTED");
        if let Some(id) = &self.current_id {
            println!("id stream handler started: {}", id)
        } else {
            println!("no id stream handler started!")
        }
        let response = ResponseToClient {
            all_messages: self.all_messages.to_string(),
            signed_in: false,
            id: "Henry".to_string(),
            message_to_client: "None".to_string(),
        };
        ctx.text(json!(response).to_string());
    }

    fn finished(&mut self, _: &mut Self::Context) {
        println!("StreamHandler FINISHED")
    }
}

/**
TODO HERE
upon receival of message
check message format and do not perform action if message format is not correct
*/

// WEBSOCKET MESSAGE HANDLING

async fn message_handler(
    received_client_message: String,
    db_pool: web::Data<SqlitePool>,
    current_id: Option<String>,
) -> String {
    // println!("client message {:?}", received_client_message);

    let from_client: FromClient = serde_json::from_str(&received_client_message)
        .expect("parsing received_client_message msg");

    let all_messages_json = get_all_messages_json(db_pool.clone()).await;

    if let Some(id) = current_id {
        println!("id message handler: {}", id)
    } else {
        println!("no id message handler!")
    }

    // println!("client object: {:?}", from_client);

    let response;

    // check if signed in

    let sent_to_client: SentToClient =
        serde_json::from_str(&from_client.message).expect("parsing from_client msg");

    // ADD TO MESSAGES
    // sqlx::query!(
    //     r#"INSERT INTO message (user_id, room_id, message, time) VALUES ($1, $2, $3, $4)"#,
    //     from_client.user_id,
    //     from_client.room_id,
    //     from_client.message,
    //     from_client.time
    // )
    // .execute(db_pool.get_ref())
    // .await
    // .expect("query insert message error");

    response = ResponseToClient {
        all_messages: all_messages_json.to_string(),
        signed_in: false,
        id: "Henry".to_string(),
        message_to_client: "None".to_string(),
    };

    json!(response).to_string()
}

// WEBSOCKET/DATABASE HELPER FUNCTIONS

async fn get_all_messages_json(db_pool: web::Data<SqlitePool>) -> Value {
    let all_messages: Vec<DatabaseMessage> =
        sqlx::query_as!(DatabaseMessage, "SELECT * FROM message ORDER BY time")
            .fetch_all(db_pool.get_ref())
            .await
            .expect("all messages failure");
    let all_messages_json = json!(&all_messages);
    all_messages_json
}

// WEBSOCKET INDEX HANDLING
/**
TODO HERE
Perform left outer join to get user name from user table
*/
async fn index(
    db_pool: web::Data<SqlitePool>,
    req: HttpRequest,
    stream: web::Payload,
    id: Identity,
    shared_hash: web::Data<Mutex<HashMap<String, Addr<WebSocketActor>>>>,
) -> Result<HttpResponse, Error> {
    // ) -> Result<(Addr<WebSocketActor>, HttpResponse), Error> {
    if let Some(id) = id.identity() {
        println!("id index: {}", id)
    } else {
        println!("no id index!")
    }

    let all_messages = get_all_messages_json(db_pool.clone()).await;

    let current_id = id.identity();

    // let response = ws::start(
    let act_with_add = ws::start_with_addr(
        WebSocketActor {
            all_messages,
            db_pool,
            current_id,
        },
        &req,
        stream,
    );

    let ws_add;
    let response;

    match act_with_add {
        Ok(res) => {
            ws_add = res.0;
            let hash_ref = shared_hash.get_ref();
            hash_ref
                .lock()
                .unwrap()
                .insert("ws_addr".to_string(), ws_add);
            response = Ok(res.1);
        }
        Err(e) => {
            println!("Err actor with add: {:?}", e);
            panic!();
        }
    }

    // act_with_add
    response
}

// let address_map = ws_addr.get_ref();
// address_map.insert("ws_addr".to_string(), ws_add);
// // AUTH HANDLING

#[derive(Serialize, Deserialize, Debug)]
struct SignInSignUp {
    user_name: String,
    password: String,
}

// sqlx::query!(
//     r#"INSERT INTO message (user_id, room_id, message, time) VALUES ($1, $2, $3, $4)"#,
//     from_client.user_id,
//     from_client.room_id,
//     from_client.message,
//     from_client.time
// )
// .execute(db_pool.get_ref())
// .await
// .expect("query insert message error");

/**
    SIGN UP
    Graceful error if receiving wrong data from frontend.
    Check database if name exists or user is already logged in
        if exists/logged in -> send response to client
    else
        -> save user_name + password in db
        -> sign-in the user
*/
async fn signup(id: Identity, req_body: String, db_pool: web::Data<SqlitePool>) -> HttpResponse {
    // println!("Signup request body: {:?}", req_body);
    // let body_json: SignInSignUp = serde_json::from_str(&req_body).expect("error in signup body");
    // let user_name = &body_json.user_name;
    // let password = body_json.password;

    // // check if user name exists
    // let user = sqlx::query!(r#"SELECT id FROM user WHERE name=$1"#, user_name)
    //     .fetch_one(db_pool.get_ref())
    //     .await;

    // match user {
    //     // If user name exists, exit the process
    //     Ok(user) => {
    //         println!("user ALREADY exists, {:?}", user);
    //         HttpResponse::Ok().finish()
    //     }
    //     // if user does NOT exist, then sign them up
    //     Err(user) => {
    //         println!("user does not exist, thus we are saving them, {:?}", user);
    //         let config = Config::default();
    //         let password_hash = argon2::hash_encoded(password.as_bytes(), SALT, &config).unwrap();

    //         // SAVE THE USER
    //         sqlx::query!(
    //             r#"INSERT INTO user (name, password) VALUES ($1, $2)"#,
    //             user_name,
    //             password_hash
    //         )
    //         .execute(db_pool.get_ref())
    //         .await
    //         .expect("Saving new user did NOT work");

    //         // save the user for this session
    //         id.remember(body_json.user_name.to_owned());
    //         HttpResponse::Ok().finish()
    //     }
    // }
    // FOR TESTING COOKIE STUFF
    if let Some(id) = id.identity() {
        println!("id signup {}", id)
    } else {
        println!("no id signup!")
    }
    HttpResponse::Ok().finish()
}
/**
    LOGIN
    Graceful error if receiving wrong data from frontend.
    Check database for user_name and password combo
        if exists -> sign in that user
    else
        -> send failed attempt message
*/
async fn login(
    id: Identity,
    req_body: String,
    db_pool: web::Data<SqlitePool>,
    shared_hash: web::Data<Mutex<HashMap<String, Addr<WebSocketActor>>>>,
    // shared_hash: web::Data<HashMap<String, Addr<WebSocketActor>>>,
) -> HttpResponse {
    // id.remember("User1".to_owned()); // <- remember identity
    let body_json: SignInSignUp = serde_json::from_str(&req_body).expect("error in login body");
    let user_name = &body_json.user_name;
    let password = &body_json.password;

    println!("Hello?");

    // Check for user name
    match sqlx::query!(r#"SELECT * FROM user WHERE name=$1"#, user_name)
        .fetch_one(db_pool.get_ref())
        .await
    {
        Ok(user_record) => {
            // println!("user exists, {:?}", user_record);
            let pw_hash = user_record.password;
            let password_match = argon2::verify_encoded(&pw_hash, password.as_bytes()).unwrap();
            println!("{:?}, {:?}", password, pw_hash);
            //check if password matches
            match password_match {
                true => {
                    // Remember the ID
                    id.remember(user_name.to_owned());
                    // Reset the WS
                    let ws_hm = shared_hash.get_ref().lock().unwrap();
                    let ws_add = ws_hm.get(&"ws_addr".to_string());

                    match ws_add {
                        Some(add) => {
                            let result = add.send(Ping).await;
                            match result {
                                Ok(res) => println!("Got reset result: {}", res.unwrap()),
                                Err(err) => println!("Got reset error: {}", err),
                            }
                        }
                        None => println!("no ws add"),
                    }
                }
                false => {}
            }

            if let Some(id) = id.identity() {
                println!("id in login: {:?}", id)
            } else {
                println!("No id login")
            }

            HttpResponse::Ok().finish()
        }
        Err(user) => {
            println!("user does not exist, thus no login, {:?}", user);
            HttpResponse::Ok().finish()
        }
    }
}

async fn logout(
    id: Identity,
    shared_hash: web::Data<Mutex<HashMap<String, Addr<WebSocketActor>>>>,
) -> HttpResponse {
    let ws_hm = shared_hash.get_ref().lock().unwrap();
    let ws_key = "ws_addr".to_string();
    let ws_add = ws_hm.get(&ws_key);

    match ws_add {
        Some(add) => println!("ws add: {:?}", add),
        None => println!("no ws add"),
    }

    // id.forget(); // <- remove identity
    // HttpResponse::Ok().finish()

    // FOR TESTING
    if let Some(id) = id.identity() {
        println!("id in login: {:?}", id)
    } else {
        println!("No id login")
    }
    HttpResponse::Ok().finish()
}

// MAIN AND DB INSTANTIATION

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // set env variables for sqlx
    dotenv().ok();

    // Saving the address
    let ws_addr: HashMap<String, Addr<WebSocketActor>> = HashMap::new();
    let shared_hash = web::Data::new(Mutex::new(ws_addr));

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

    // GENERATE A RANDOM 32 BYTE KEY.
    // Note that it is important to use a unique
    // private key for every project. Anyone with access to the key can generate
    // authentication cookies for any user!
    let private_key = rand::thread_rng().gen::<[u8; 32]>();

    HttpServer::new(move || {
        App::new()
            .wrap(
                // create identity middleware
                IdentityService::new(
                    // create cookie identity policy
                    CookieIdentityPolicy::new(&private_key)
                        .name("auth-cookie")
                        .secure(true)
                        // NOT FOR
                        .same_site(actix_web::cookie::SameSite::None),
                ),
            )
            .wrap(Logger::default())
            // pass a clone of the pool to the request
            .app_data(shared_db_pool.clone())
            .app_data(shared_hash.clone())
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

// PW DEMO
// let password = b"password";
// let config = Config::default();
// let hash = argon2::hash_encoded(password, SALT, &config).unwrap();
// let matches = argon2::verify_encoded(&hash, password).unwrap();
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
