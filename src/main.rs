use actix::{Actor, Running, StreamHandler};
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use chrono;
use dotenv::dotenv;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::sqlite::SqlitePoolOptions;
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

struct TheWebSocket;

impl Actor for TheWebSocket {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.text("HEY OVER HERE".to_string());
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        println!("STOPPING HERE");
        Running::Stop
    }
}

fn text_handler(text: &String) -> &str {
    match text.as_str() {
        "hey" => "hello",
        "dog" => "cat",
        x => x,
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for TheWebSocket {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => ctx.text(text_handler(&text)),
            Ok(ws::Message::Binary(_)) => println!("Unexpected binary"),
            _ => (),
        }
    }
}

/**
async fn index(pool: web::Data<SqlitePool>, req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    let resp = ws::start(TheWebSocket { pool: pool.clone() }, &req, stream); // Clone the pool into the actor
    println!("{:?}", resp);
    resp
}
*/

/**
If you just start the ws actor and send some messages once that's one time.
let (addr, res) = ws::start_with_addr();
let json = db.query().await;
addr.send(Msg(json)).await;
res
This is what you want to do with onetime db query and send message to client
*/

/**
impl StreamHandler<Msg> for Act {
    fn handle(&mut self, msg: Msg, ctx: &mut Self::Context) {
        self.db.query().into_actor(self).spawn(ctx);
    }
}
If you does something like this. It's always preferred to let your actor hold a db object
*/

async fn index(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    let resp = ws::start(TheWebSocket {}, &req, stream);
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
    dotenv().ok();

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite://./chat.db")
        .await
        .expect("pool FAILURE");

    let all_messages: Vec<Message> = sqlx::query_as!(Message, "SELECT * from message")
        .fetch_all(&pool)
        .await
        .expect("all messages failure");

    let all_messages_json = json!(&all_messages);

    println!("all_messages: {:?}", all_messages_json);

    HttpServer::new(|| App::new().route("/ws/", web::get().to(index)))
        .bind("127.0.0.1:8081")?
        .run()
        .await
}
