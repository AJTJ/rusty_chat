```rs
#[derive(Debug)]
struct WebSocketActor {
    db_pool: web::Data<SqlitePool>,
    // SWITCH THESE POTENTIALLY?
    // open_sockets_data: web::Data<Mutex<HashMap<SocketId, OpenSocketData>>>,
    open_sockets_data: web::Data<RwLock<HashMap<SocketId, OpenSocketData>>>,
    hb: Instant,
    // SWITCH THESE POTENTIALLY?
    //  session_table_data: web::Data<Mutex<HashMap<SessionID, SessionData>>>,
    session_table_data: web::Data<RwLock<HashMap<SessionID, SessionData>>>,
    signed_in_user: String,
    session_id: String,
    socket_id: [u8; 32],
}
```
NOTES
- Not much of a difference in the majority of cases
- It could also be a slow down if RwLock is not fair.
- For a chat app when you remove a session it should never wait for readers. So at least a RwLock biased to writer is prefered.
- This is especially noticeable when bandwidth is what matters to you. An RwLock will wait for readers could lead to a supposed going away connection to be sent with extra messages it does not need. 