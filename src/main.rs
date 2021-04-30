use actix::{Actor, StreamHandler};
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use chrono;
use dotenv::dotenv;
use serde_json::json;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::FromRow;

struct TheWebSocket;

impl Actor for TheWebSocket {
    type Context = ws::WebsocketContext<Self>;
}

fn text_handler(text: &String) -> &str {
    match text.as_str() {
        "hey" => "hello",
        "dog" => "cat",
        x => x,
    }
}

// let active_messages: Vec<MessageBlock> = Vec::new();

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for TheWebSocket {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => ctx.text(text_handler(&text)),
            Ok(ws::Message::Binary(bin)) => ctx.binary(bin),
            _ => (),
        }
    }
}

async fn index(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    let resp = ws::start(TheWebSocket {}, &req, stream);
    println!("{:?}", resp);
    resp
}

//db things
// basic message
// 666, 123, 456, "A simple message", "2013-10-07 08:23:19.120";

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();

    let test_data = json!({
        "hello": "this"
    });

    println!("{:?}", test_data);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite://./chat.db")
        .await
        .expect("pool FAILURE");

    #[derive(Debug)]
    struct User {
        id: Option<i64>,
        name: Option<String>,
    }

    #[derive(Debug)]
    struct Message {
        id: Option<i64>,
        user_id: i64,
        room_id: i64,
        message: String,
        time: chrono::NaiveDateTime,
    }
    #[derive(Debug)]
    struct Room {
        id: Option<i64>,
        name: Option<String>,
    }

    let all_messages: Vec<Message> = sqlx::query_as!(Message, "SELECT * from message")
        .fetch_all(&pool)
        .await
        .expect("dang thing");

    println!("pool: {:?}", pool);
    println!("all_messages: {:?}", all_messages);

    HttpServer::new(|| App::new().route("/ws/", web::get().to(index)))
        .bind("127.0.0.1:8081")?
        .run()
        .await
}
