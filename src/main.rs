use actix::{Actor, StreamHandler};
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
// use mysql::prelude::*;
// use mysql::*;

struct MessageBlock {
    user_name: String,
    user_id: String,
    message: String,
    message_id: String,
}

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

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| App::new().route("/ws/", web::get().to(index)))
        .bind("127.0.0.1:8081")?
        .run()
        .await
}
