// ACTIX
use actix_web::{middleware::Logger, web, App, HttpServer};

// UTILS
use dotenv::{dotenv, var};

// DB
use diesel::pg::PgConnection;
use diesel::r2d2::{self, ConnectionManager};

// STD
use std::collections::HashMap;
use std::sync::Mutex;

// MODS
use rusty_chat::auth::{login, logout, signup};
use rusty_chat::dto::{OpenSocketData, SessionData, SessionID, UniversalIdType};
use rusty_chat::socket_actor::ws_index;

// UNUSED
use actix_cors::Cors;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // println!("Server running");
    // ENV
    dotenv().ok();
    let dev_key = "DEVELOPMENT";
    let env_dev = var(dev_key);

    // let db_key = "DATABASE_URL";
    // let env_db = var(db_key).unwrap();
    // let env_db_slice: &str = &*env_db;

    let mut local_url = "0.0.0.0:8081".to_string();
    if let Ok(x) = env_dev {
        local_url = x;
    }

    println!("Serving at: {}", &local_url);

    // OPEN SOCKETS DATA
    let socket_data_hashmap: HashMap<UniversalIdType, OpenSocketData> = HashMap::new();
    let open_sockets_data = web::Data::new(Mutex::new(socket_data_hashmap));

    // SESSION TABLE
    let session_table_hashmap: HashMap<SessionID, SessionData> = HashMap::new();
    let session_table_data = web::Data::new(Mutex::new(session_table_hashmap));

    // DATABASE
    let connspec = var("DATABASE_URL").expect("DATABASE_URL");
    let manager = ConnectionManager::<PgConnection>::new(connspec);
    let shared_db_pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool.");

    HttpServer::new(move || {
        let cors = Cors::permissive();
        App::new()
            .wrap(cors)
            .wrap(Logger::default())
            // DATA
            .app_data(shared_db_pool.clone())
            .app_data(open_sockets_data.clone())
            .app_data(session_table_data.clone())
            // ENDPOINTS
            .route("/signup/", web::post().to(signup))
            .route("/login/", web::post().to(login))
            .route("/logout/", web::get().to(logout))
            .route("/ws/", web::get().to(ws_index))
    })
    .bind(&local_url)?
    .run()
    .await
}
