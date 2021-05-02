
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

...

the error I'm getting is 
the trait bound `&actix_web::web::Data<Pool<Sqlite>>: Executor<'_>` is not satisfied
the trait `Executor<'_>` is not implemented for `&actix_web::web::Data<Pool<Sqlite>>`

fakeshadow — Today at 11:12 AM
db_pool.get_ref()
error is pretty clear. You are trying to pass Data<Pool> to sqlx
It knows nothing about Data type and it needs &Pool
You always want get the type right when compiler tells you to do so.
AJTJ — Today at 11:15 AM
I actually don't understand that error message. How were you able to infer that solution?
fakeshadow — Today at 11:16 AM
trait bound not satisfied means .fetch_all takes a generic type. which is the pool type here.
the trait Executor<'_> is not implemented for `&actix_web::web::Data<Pool<Sqlite>> tells you the generic type must impl Executor trait.
Then it means web::Data must prevent the trait bound as Data is an actix-web type it can not check what inside have certain trait bound or not
by using get_ref you dereference Data type and get a &Pool<Sqlite>. Which is a sqlx type
So it's highly likely impl Executor trait.
AJTJ — Today at 11:21 AM
so, just to reiterate:
trait bound "x" is not satisfied, means that "x" is not the type desired?
the trait "y" not implemented makes sense.
fakeshadow — Today at 11:21 AM
trait impls on types. That's what we call bound
When something ask for a trait bound. It's just asking for some type impl the trait.

---

- I guess your actors should share a pool of connections, in started you can perform an sqlx query and then send the message to the client
- In started the self parameter is your TheWebSocket, notice, that it's currently empty and in index you initialize it
- ws::start(TheWebSocket {}, &req, stream);
- https://actix.rs/docs/databases/ here you can borrow the idea of sharing the pool between your actors
- EXAMPLE IN CODE
- ws::start_with_addr would return actor address to you
- You can use it to send message to the actor.
- passing a db object to it only make sense when you want to do a lot of db query with ws actor.