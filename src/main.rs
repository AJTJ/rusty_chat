use actix::{Actor, Running, StreamHandler};
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use chrono;
use dotenv::dotenv;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{sqlite::SqlitePoolOptions, Sqlite};
use sqlx::{Pool, SqlitePool};
// use sqlx::FromRow;

#[derive(Serialize, Deserialize, Debug)]
struct User {
    id: Option<i64>,
    name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Message {
    id: Option<i64>,
    user_id: i64,
    room_id: i64,
    message: String,
    time: chrono::NaiveDateTime,
}
#[derive(Serialize, Deserialize, Debug)]
struct Room {
    id: Option<i64>,
    name: Option<String>,
}

struct WebSockActor {
    all_messages: serde_json::Value,
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
add current time object to received message
save message in database
*/

fn message_handler(json_message: &String) -> &str {
    println!("{:?}", json_message);
    let mes: Value = serde_json::from_str(&json_message).expect("parsing json msg");
    println!("{:?}", mes);

    r#"[{"message": 43}]"#
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WebSockActor {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(json_message)) => ctx.text(message_handler(&json_message)),
            Ok(ws::Message::Binary(_)) => println!("Unexpected binary"),
            _ => (),
        }
    }
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
    let all_messages: Vec<Message> = sqlx::query_as!(Message, "SELECT * from message")
        .fetch_all(db_pool.get_ref())
        .await
        .expect("all messages failure");

    let all_messages_json = json!(&all_messages);

    println!("all_messages: {:?}", all_messages_json);

    let resp = ws::start(
        WebSockActor {
            all_messages: all_messages_json,
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
