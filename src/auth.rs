use actix_web::HttpMessage;
use actix_web::{cookie, web, HttpRequest, HttpResponse};
use argon2::{self, Config};
use chrono::{prelude::*, Duration};
use diesel::pg::PgConnection;
use diesel::r2d2::{self, ConnectionManager};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::convert::TryInto;
use std::sync::Mutex;
use time::{Duration as TimeDuration, OffsetDateTime};

// MODS
use crate::dto::{
    CookieStruct, OpenSocketData, SessionData, SessionID, UniversalIdType, COOKIE_NAME,
};

use crate::socket_actor::resend_ws;

type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

// AUTH HANDLING
#[derive(Serialize, Deserialize, Debug)]
pub struct SignInSignUp {
    user_name: String,
    password: String,
}

// SIGN UP
pub async fn signup(
    req_body: String,
    db_pool: web::Data<DbPool>,
    session_table_data: web::Data<Mutex<HashMap<SessionID, SessionData>>>,
    req: HttpRequest,
) -> HttpResponse {
    // I/O DATA
    let body_json: SignInSignUp =
        serde_json::from_str(&req_body).expect("Error in Client msg to sign in");
    let user_name = &body_json.user_name;
    let password = body_json.password;

    if user_name.is_empty() || password.is_empty() {
        return HttpResponse::Ok().body(json!("Please type a name/password".to_string()));
    }

    // CHECK FOR USER IN DB
    let user = sqlx::query!(r#"SELECT id FROM user WHERE name=$1"#, user_name)
        .fetch_one(db_pool.get_ref())
        .await;

    match user {
        // USER EXISTS -> EXIT
        Ok(_) => HttpResponse::Ok().body(json!("User already exists".to_string())),
        // NO USER -> SIGNUP & SIGNIN
        Err(_) => {
            // SAVE THE USER_NAME, PW, SALT
            let config = Config::default();
            let salt_gen: UniversalIdType = rand::thread_rng().gen::<UniversalIdType>();
            let salt: &[u8] = &salt_gen[..];
            let password_hash = argon2::hash_encoded(password.as_bytes(), salt, &config).unwrap();
            sqlx::query!(
                r#"INSERT INTO user (name, hash, salt) VALUES ($1, $2, $3)"#,
                user_name,
                password_hash,
                salt
            )
            .execute(db_pool.get_ref())
            .await
            .expect("Saving new user did NOT work");

            // LOGIN
            login_process(session_table_data, req, user_name)
        }
    }
}

pub fn login_process(
    session_table_data: web::Data<Mutex<HashMap<SessionID, SessionData>>>,
    req: HttpRequest,
    user_name: &str,
) -> HttpResponse {
    let session_table_ref = session_table_data.get_ref();
    let mut session_table = session_table_ref.lock().unwrap();

    // CREATE SESSION ID
    let id: UniversalIdType = rand::thread_rng().gen::<UniversalIdType>();
    let encoded_session_id = base64::encode(id);

    // BASIC CLEANUP
    session_table.retain(|_, session| session.expiry > Utc::now().naive_utc());

    // CHECK IF SIGNED IN ELSEWHERE/ALREADY
    // if session_table.values().any(|x| &x.user_name == user_name) {
    //     return HttpResponse::Ok().body(json!(format!("{} is already signed in", user_name)));
    // };

    // IF A COOKIE IS PRESENT - REMOVE IT FROM SESSION
    let cookie_option = req.cookie(COOKIE_NAME);

    if let Some(cookie) = cookie_option {
        let (_, value) = cookie.name_value();
        let cookie_data: CookieStruct = serde_json::from_str(value).expect("parsing cookie error");
        session_table.remove_entry(&cookie_data.id);
    }

    // ADD TO SESSION TABLE
    let current_session_data = SessionData {
        user_name: user_name.to_string(),
        expiry: Utc::now().naive_utc() + Duration::minutes(5),
    };
    session_table.insert(encoded_session_id.clone(), current_session_data);

    // CREATE NEW COOKIE
    let cookie_values = CookieStruct {
        id: encoded_session_id,
        user_name: user_name.to_string(),
    };
    let cookie = cookie::Cookie::build(COOKIE_NAME, json!(cookie_values).to_string())
        .path("/")
        .secure(true)
        .max_age(TimeDuration::minutes(30))
        .same_site(cookie::SameSite::None)
        .http_only(true)
        .finish();

    println!("login process complete");
    // RETURN AND ADD/REWRITE COOKIE

    HttpResponse::Ok()
        .cookie(cookie)
        .body(json!("Welcome".to_string()))
}

// LOGIN
pub async fn login(
    req_body: String,
    db_pool: web::Data<DbPool>,
    session_table_data: web::Data<Mutex<HashMap<SessionID, SessionData>>>,
    req: HttpRequest,
) -> HttpResponse {
    // INCOMING DATA
    let body_json: SignInSignUp = serde_json::from_str(&req_body).expect("error in login body");
    let user_name = &body_json.user_name;
    let password = &body_json.password;

    // CHECK USER NAME
    match sqlx::query!(r#"SELECT * FROM user WHERE name=$1"#, user_name)
        .fetch_one(db_pool.get_ref())
        .await
    {
        Ok(user_record) => {
            // CHECK PASSWORD
            let pw_hash = user_record.hash;
            let password_match = argon2::verify_encoded(&pw_hash, password.as_bytes()).unwrap();
            match password_match {
                // CORRECT PW AND LOGIN
                true => {
                    println!("correct user/pw combo: {}", &user_name);
                    login_process(session_table_data, req, user_name)
                }
                // WRONG PASSWORD
                false => HttpResponse::Ok().body(json!(format!(
                    "Password does not match the user: {}",
                    user_name
                ))),
            }
        }
        // USER NOT IN DB
        Err(_) => {
            HttpResponse::Ok().body(json!("User does not exist, please register.".to_string()))
        }
    }
}

pub async fn logout(
    req: HttpRequest,
    session_table_data: web::Data<Mutex<HashMap<SessionID, SessionData>>>,
    open_sockets_data: web::Data<Mutex<HashMap<UniversalIdType, OpenSocketData>>>,
) -> HttpResponse {
    let cookie_option = req.cookie(COOKIE_NAME);
    match cookie_option {
        Some(cookie) => {
            // REMOVE FROM SESSION
            let session_table_ref = session_table_data.get_ref();
            let mut session_table = session_table_ref.lock().unwrap();
            let (_, value) = cookie.name_value();
            let cookie_data: CookieStruct =
                serde_json::from_str(value).expect("parsing cookie error");
            session_table.remove_entry(&cookie_data.id);

            let decoded_id_vec = base64::decode(&cookie_data.id).unwrap();
            let decoded_id: UniversalIdType = decoded_id_vec.as_slice().try_into().unwrap();

            // REMOVE CURRENT SOCKET FROM SOCKETS
            let open_sockets_data_ref = open_sockets_data.get_ref();
            open_sockets_data_ref
                .lock()
                .unwrap()
                .remove_entry(&decoded_id);

            // SEND A REFRESH TO OTHER WS ACTORS TO UPDATE THEIR USER LIST IMMEDIATELY
            resend_ws(open_sockets_data.clone()).await;

            // REMOVE COOKIE BY REPLACING WITH ALREADY EXPIRED COOKIE
            let cookie = cookie::Cookie::build(COOKIE_NAME, "should_be_expired".to_string())
                .path("/")
                .secure(false)
                .expires(OffsetDateTime::unix_epoch())
                .http_only(true)
                .finish();

            HttpResponse::Ok().cookie(cookie).finish()
        }
        None => HttpResponse::Ok().finish(),
    }
}
