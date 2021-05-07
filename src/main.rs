use actix::{Actor, ActorFuture, ContextFutureSpawner, Running, StreamHandler, WrapFuture};
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use chrono;
use chrono::prelude::*;
use dotenv::dotenv;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{sqlite::SqlitePoolOptions, Sqlite};
use sqlx::{Pool, SqlitePool};
// use sqlx::FromRow;

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
struct Room {
    id: i64,
    name: String,
}

struct WebSockActor {
    all_messages: serde_json::Value,
    db_pool: web::Data<SqlitePool>,
}

impl Actor for WebSockActor {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        println!("STARTING HERE");
        ctx.text(self.all_messages.to_string());
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        println!("STOPPING HERE");
        Running::Stop
    }
}

/**
TODO HERE
upon receival of message
program should not fail if message is not correct format
if message has all correct data, then save the message in database
Currently the front end sets the user_id, room_id and message
*/

async fn message_handler(client_message: String, db_pool: web::Data<SqlitePool>) -> String {
    println!("JSON VALUE {:?}", client_message);
    let mes: ClientMessage = serde_json::from_str(&client_message).expect("parsing json msg");

    sqlx::query!(
        r#"INSERT INTO message (user_id, room_id, message, time) VALUES ($1, $2, $3, $4)"#,
        mes.user_id,
        mes.room_id,
        mes.message,
        mes.time
    )
    .execute(db_pool.get_ref())
    .await
    .expect("query insert message error");

    let all_messages = get_all_messages(db_pool.clone()).await;

    let all_messages_json = json!(&all_messages);

    all_messages_json.to_string()
}

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
}

async fn get_all_messages(db_pool: web::Data<SqlitePool>) -> Vec<DatabaseMessage> {
    let all_messages: Vec<DatabaseMessage> =
        sqlx::query_as!(DatabaseMessage, "SELECT * FROM message ORDER BY time")
            .fetch_all(db_pool.get_ref())
            .await
            .expect("all messages failure");
    all_messages
}

/**
TODO HERE
Perform left outer join to get user name from user table
*/
async fn index(
    db_pool: web::Data<SqlitePool>,
    req: HttpRequest,
    stream: web::Payload,
) -> Result<HttpResponse, Error> {
    let all_messages = get_all_messages(db_pool.clone()).await;

    let all_messages_json = json!(&all_messages);

    println!("all_messages: {:?}", all_messages_json);

    let resp = ws::start(
        WebSockActor {
            all_messages: all_messages_json,
            db_pool,
        },
        &req,
        stream,
    );
    println!("{:?}", resp);
    resp
}
/**
LOGICAL PROCESS
1. serve all messages to all actors
2. await receipt of new message
3. update db with new message
4. update all connections with updated message list
*/

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
            // pass a clone of the pool to the request
            .app_data(shared_db_pool.clone())
            .route("/ws/", web::get().to(index))
    })
    .bind("127.0.0.1:8081")?
    .run()
    .await
}
