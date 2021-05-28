use actix::prelude::*;
use actix::{Actor, StreamHandler};
use actix_web_actors::ws;
use log::{debug, warn};
use std::time::Duration;
use tokio::sync::broadcast::Receiver;

// Define HTTP actor
pub struct ChangesWs(pub Option<Receiver<i32>>);

#[derive(Message)]
#[rtype(result = "()")]
pub struct ChangeEvent(pub String);

impl Handler<ChangeEvent> for ChangesWs {
    type Result = ();

    fn handle(&mut self, msg: ChangeEvent, ctx: &mut Self::Context) {
        ctx.text(format!("reload: {}", msg.0).to_string());
    }
}

impl Actor for ChangesWs {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(std::time::Duration::from_secs(1), |_, ctx| {
            ctx.ping(b"ping");
        });
        // start heartbeats otherwise server will disconnect after 10 seconds
        //let mut watcher = watcher(tx, Duration::from_secs(10)).unwrap();
        //watcher.watch("/opt/ocs/public", RecursiveMode::Recursive).unwrap();
        //self.0 = Some(watcher);
        let rx = std::mem::replace(&mut self.0, None);
        if let Some(mut rx) = rx {
            ctx.run_interval(Duration::from_millis(100), move |_act, ctx| {
                match rx.try_recv() {
                    Ok(_event) => {
                        debug!("reload");
                        ctx.text("reload");
                    }
                    Err(_) => {}
                }
            });
        }
    }

    fn stopped(&mut self, _: &mut ws::WebsocketContext<Self>) {
        warn!("Disconnected");
    }
}

/// Handler for ws::Message message
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for ChangesWs {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        debug!("got ws message: {:?}", msg);
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => ctx.text(text),
            Ok(ws::Message::Binary(bin)) => ctx.binary(bin),
            Ok(ws::Message::Close(_)) => ctx.stop(),
            _ => (),
        }
    }
}
