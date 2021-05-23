use actix::{Actor, ActorFuture, ContextFutureSpawner, Running, StreamHandler, WrapFuture};
use actix_cors::Cors;
use actix_identity::{CookieIdentityPolicy, Identity, IdentityService};
use actix_web::{
    http::header, middleware::Logger, web, App, Error, HttpRequest, HttpResponse, HttpServer,
};
use actix_web_actors::ws;
use argon2::{self, Config};
use chrono;
use chrono::prelude::*;
use dotenv::dotenv;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
// use actix_web::{http::header, middleware::Logger, App, HttpServer};
// use sqlx::FromRow;

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
struct ClientResponse {
    message: String,
    is_sign_in: bool,
    is_sign_up: bool,
    user_name: String,
    password: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ClientMessage {
    user_id: i64,
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

struct WebSockActor {
    all_messages: serde_json::Value,
    db_pool: web::Data<SqlitePool>,
    id: Identity,
}

// WS ACTOR INSTANTIATION

impl Actor for WebSockActor {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, _: &mut Self::Context) {
        println!("WebSocket Actor Started");
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        println!("WebSocket Actor Closed");
        Running::Stop
    }
}

// WS STREAM HANDLER

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WebSockActor {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(json_message)) => {
                message_handler(json_message, self.db_pool.clone())
                    .into_actor(self)
                    .map(|text, _, ctx| ctx.text(text))
                    .wait(ctx)
            }
            Ok(ws::Message::Binary(_)) => println!("Unexpected binary"),
            _ => (),
        }
    }

    fn started(&mut self, ctx: &mut Self::Context) {
        println!("StreamHandler started");
        let response = ResponseToClient {
            all_messages: self.all_messages.to_string(),
            signed_in: false,
            id: "Henry".to_string(),
            message_to_client: "None".to_string(),
        };
        ctx.text(json!(response).to_string());
    }

    fn finished(&mut self, _: &mut Self::Context) {
        self.id.forget();
        println!("StreamHandler finished")
    }
}

/**
TODO HERE
upon receival of message
check message format and do not perform action if message format is not correct
*/

// MESSAGE HANDLING

async fn message_handler(
    received_client_message: String,
    db_pool: web::Data<SqlitePool>,
) -> String {
    println!("client message {:?}", received_client_message);

    let client_object: ClientResponse = serde_json::from_str(&received_client_message)
        .expect("parsing received_client_message msg");

    let all_messages_json = get_all_messages_json(db_pool.clone()).await;

    println!("client object: {:?}", client_object);

    let response;

    if client_object.is_sign_up == true {
        println!("Is sign up");

        response = ResponseToClient {
            all_messages: all_messages_json.to_string(),
            signed_in: false,
            id: "Henry".to_string(),
            message_to_client: "Performing sign up".to_string(),
        };
        return json!(response).to_string();
    } else if client_object.is_sign_in == true {
        println!("Is sign in");

        response = ResponseToClient {
            all_messages: all_messages_json.to_string(),
            signed_in: false,
            id: "Henry".to_string(),
            message_to_client: "Performing sign in".to_string(),
        };
        return json!(response).to_string();
    } else {
        // check if signed in
        println!("Is neither sign up nor sign in");
    }

    let client_message: ClientMessage =
        serde_json::from_str(&client_object.message).expect("parsing client_object msg");

    println!("client message: {:?}", client_message);

    // sqlx::query!(
    //     r#"INSERT INTO message (user_id, room_id, message, time) VALUES ($1, $2, $3, $4)"#,
    //     client_object.user_id,
    //     client_object.room_id,
    //     client_object.message,
    //     client_object.time
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

// HELPER FUNCTIONS

async fn get_all_messages_json(db_pool: web::Data<SqlitePool>) -> Value {
    let all_messages: Vec<DatabaseMessage> =
        sqlx::query_as!(DatabaseMessage, "SELECT * FROM message ORDER BY time")
            .fetch_all(db_pool.get_ref())
            .await
            .expect("all messages failure");
    let all_messages_json = json!(&all_messages);
    all_messages_json
}

// INDEX HANDLING
/**
TODO HERE
Perform left outer join to get user name from user table
*/
async fn index(
    db_pool: web::Data<SqlitePool>,
    req: HttpRequest,
    stream: web::Payload,
    id: Identity,
) -> Result<HttpResponse, Error> {
    let resp = ws::start(
        WebSockActor {
            all_messages: get_all_messages_json(db_pool.clone()).await,
            db_pool,
            id,
        },
        &req,
        stream,
    );
    // println!("{:?}", resp);
    resp
}

// AUTH HANDLING

#[derive(Serialize, Deserialize, Debug)]
struct SignInSignUp {
    user_name: String,
    password: String,
}

// sqlx::query!(
//     r#"INSERT INTO message (user_id, room_id, message, time) VALUES ($1, $2, $3, $4)"#,
//     client_object.user_id,
//     client_object.room_id,
//     client_object.message,
//     client_object.time
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
    println!("Signup request body: {:?}", req_body);
    let body_json: SignInSignUp = serde_json::from_str(&req_body).expect("error in signup body");
    let user_name = &body_json.user_name;
    let password = body_json.password;

    // check if user name exists
    let user = sqlx::query!(r#"SELECT id FROM user WHERE name=$1"#, user_name)
        .fetch_one(db_pool.get_ref())
        .await;

    match user {
        // If user name exists, exit the process
        Ok(user) => {
            println!("user ALREADY exists, {:?}", user);
            HttpResponse::Ok().finish()
        }
        // if user does NOT exist, then sign them up
        Err(user) => {
            println!("user does not exist, thus we are saving them, {:?}", user);
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
            id.remember(body_json.user_name.to_owned());
            HttpResponse::Ok().finish()
        }
    }
}
/**
    LOGIN
    Graceful error if receiving wrong data from frontend.
    Check database for user_name and password combo
        if exists -> sign in that user
    else
        -> send failed attempt message
*/
async fn login(id: Identity, req_body: String, db_pool: web::Data<SqlitePool>) -> HttpResponse {
    // id.remember("User1".to_owned()); // <- remember identity
    let body_json: SignInSignUp = serde_json::from_str(&req_body).expect("error in login body");
    let user_name = &body_json.user_name;
    let password = &body_json.password;

    // Check for user name
    match sqlx::query!(r#"SELECT * FROM user WHERE name=$1"#, user_name)
        .fetch_one(db_pool.get_ref())
        .await
    {
        Ok(user_record) => {
            println!("user exists, {:?}", user_record);
            let pw_hash = user_record.password;
            let password_match = argon2::verify_encoded(&pw_hash, password.as_bytes());
            println!("{:?}, {:?}", password, pw_hash);
            //check if password matches
            match password_match {
                Ok(_) => {
                    // sign in user
                    println!("password matches");
                    id.remember(user_name.to_owned());
                }
                // don't sign in if not matching
                Err(e) => println!("password not match, {:?}", e),
            };
            HttpResponse::Ok().finish()
        }
        Err(user) => {
            println!("user does not exist, thus no login, {:?}", user);
            HttpResponse::Ok().finish()
        }
    }
}

async fn logout(id: Identity) -> HttpResponse {
    id.forget(); // <- remove identity
    HttpResponse::Ok().finish()
}

// MAIN AND DB INSTANTIATION

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let password = b"password";

    let config = Config::default();
    let hash = argon2::hash_encoded(password, SALT, &config).unwrap();
    let matches = argon2::verify_encoded(&hash, password).unwrap();
    assert!(matches);

    // set env variables for sqlx
    dotenv().ok();

    // pool constructed here for all actors
    let db_pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite://./chat.db")
        .await
        .expect("pool FAILURE");

    let shared_db_pool = web::Data::new(db_pool);

    // Generate a random 32 byte key. Note that it is important to use a unique
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
                        .secure(false),
                ),
            )
            .wrap(
                Cors::default()
                    .allowed_origin("http://localhost:3000")
                    .allowed_methods(vec!["GET", "POST"])
                    .allowed_headers(vec![header::AUTHORIZATION, header::ACCEPT])
                    .allowed_header(header::CONTENT_TYPE)
                    .supports_credentials()
                    .max_age(3600),
            )
            .wrap(Logger::default())
            // pass a clone of the pool to the request
            .app_data(shared_db_pool.clone())
            // .service(web::resource("/index.html").to(index))
            .route("/signup/", web::post().to(signup))
            .route("/login/", web::post().to(login))
            .route("/logout/", web::get().to(logout))
            .route("/ws/", web::get().to(index))
    })
    .bind("127.0.0.1:8081")?
    .run()
    .await
}
