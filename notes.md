
async fn index(pool: web::Data<SqlitePool>, req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    let resp = ws::start(WebSockActor { pool: pool.clone() }, &req, stream); // Clone the pool into the actor
    println!("{:?}", resp);
    resp
}

If you just start the ws actor and send some messages once that's one time.
let (addr, res) = ws::start_with_addr();
let json = db.query().await;
addr.send(Msg(json)).await;
res
This is what you want to do with onetime db query and send message to client

impl StreamHandler<Msg> for Act {
    fn handle(&mut self, msg: Msg, ctx: &mut Self::Context) {
        self.db.query().into_actor(self).spawn(ctx);
    }
}
If you does something like this. It's always preferred to let your actor hold a db object
