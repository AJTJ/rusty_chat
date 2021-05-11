use actix::{Actor, ActorFuture, ContextFutureSpawner, Running, StreamHandler, WrapFuture};
use actix_identity::{CookieIdentityPolicy, Identity, IdentityService};
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use chrono;
use chrono::prelude::*;
use dotenv::dotenv;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
// use sqlx::FromRow;

// DATA STRUCTS

#[derive(Serialize, Deserialize, Debug)]
struct User {
    id: i64,
    name: String,
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
    message: ClientMessage,
    is_sign_in: bool,
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

// ACTOR INSTATIATION

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

// STREAM HANDLER

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

async fn message_handler(client_message: String, db_pool: web::Data<SqlitePool>) -> String {
    println!("client message {:?}", client_message);
    let client_object: ClientResponse =
        serde_json::from_str(&client_message).expect("parsing json msg");

    // println!("client object: {:?}", client_object);

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

    let all_messages_json = get_all_messages_json(db_pool.clone()).await;

    let response = ResponseToClient {
        all_messages: all_messages_json.to_string(),
        signed_in: false,
        id: "Henry".to_string(),
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

// POTENTIAL LOGIN HANDLING

// async fn login(id: Identity) -> HttpResponse {
//     id.remember("User1".to_owned()); // <- remember identity
//     HttpResponse::Ok().finish()
// }

// async fn logout(id: Identity) -> HttpResponse {
//     id.forget(); // <- remove identity
//     HttpResponse::Ok().finish()
// }

/**
LOGICAL PROCESS
1. serve all messages to all actors
2. await receipt of new message
3. update db with new message
4. update all connections with updated message list
*/

// MAIN AND DB INSTANTIATION

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // set env variables for sqlx
    dotenv().ok();

    // pool constructed here for all actors
    let db_pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite://./chat.db")
        .await
        .expect("pool FAILURE");

    let shared_db_pool = web::Data::new(db_pool);

    HttpServer::new(move || {
        App::new()
            .wrap(
                // create identity middleware
                IdentityService::new(
                    // create cookie identity policy
                    CookieIdentityPolicy::new(&[0; 32])
                        .name("auth-cookie")
                        .secure(false),
                ),
            )
            // pass a clone of the pool to the request
            .app_data(shared_db_pool.clone())
            // .service(web::resource("/index.html").to(index))
            // .service(web::resource("/login.html").to(login))
            .route("/ws/", web::get().to(index))
    })
    .bind("127.0.0.1:8081")?
    .run()
    .await
}
