use actix_web::HttpMessage;
use actix_web::{cookie, web, HttpRequest, HttpResponse};
use argon2::{self, Config};
use chrono::{prelude::*, Duration};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Mutex;
use time::{Duration as TimeDuration, OffsetDateTime};

// MODS
use crate::common::{SessionID, SocketId, COOKIE_NAME};
use crate::dto::{CookieStruct, OpenSocketData, SessionData};

// AUTH HANDLING
#[derive(Serialize, Deserialize, Debug)]
pub struct SignInSignUp {
    user_name: String,
    password: String,
}

// SIGN UP
pub async fn signup(
    req_body: String,
    db_pool: web::Data<SqlitePool>,
    session_table_data: web::Data<Mutex<HashMap<SessionID, SessionData>>>,
    req: HttpRequest,
) -> HttpResponse {
    // I/O DATA
    let body_json: SignInSignUp =
        serde_json::from_str(&req_body).expect("Error in Client msg to sign in");
    let user_name = &body_json.user_name;
    let password = body_json.password;

    if user_name.is_empty() || password.is_empty() {
        return HttpResponse::Ok().body(json!(format!("Please type a name/password")));
    }

    // CHECK FOR USER IN DB
    let user = sqlx::query!(r#"SELECT id FROM user WHERE name=$1"#, user_name)
        .fetch_one(db_pool.get_ref())
        .await;

    match user {
        // USER EXISTS -> EXIT
        Ok(_) => HttpResponse::Ok().body(json!(format!("User already exists"))),
        // NO USER -> SIGNUP & SIGNIN
        Err(_) => {
            // SAVE THE USER_NAME, PW, SALT
            let config = Config::default();
            let salt_gen = rand::thread_rng().gen::<[u8; 16]>();
            let salt: &[u8] = &salt_gen[..];
            let password_hash = argon2::hash_encoded(password.as_bytes(), &salt, &config).unwrap();
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
    user_name: &String,
) -> HttpResponse {
    let session_table_ref = session_table_data.get_ref();
    let mut session_table = session_table_ref.lock().unwrap();

    // CREATE SESSION ID
    let id = rand::thread_rng().gen::<[u8; 16]>();
    let encoded_session_id = base64::encode(id);

    // BASIC CLEANUP
    session_table.retain(|_, session| session.expiry > Utc::now().naive_utc());

    // CHECK IF SIGNED IN ELSEWHERE/ALREADY
    if session_table.values().any(|x| &x.user_name == user_name) {
        return HttpResponse::Ok().body(json!(format!("{} is already signed in", user_name)));
    };

    // IF A COOKIE IS PRESENT - REMOVE IT FROM SESSION
    let cookie_option = req.cookie(COOKIE_NAME);
    match cookie_option {
        Some(cookie) => {
            let (_, value) = cookie.name_value();
            let cookie_data: CookieStruct =
                serde_json::from_str(value).expect("parsing cookie error");
            session_table.remove_entry(&cookie_data.id);
        }
        None => {}
    }

    // ADD TO SESSION TABLE
    let current_session_data = SessionData {
        user_name: user_name.clone(),
        expiry: Utc::now().naive_utc() + Duration::minutes(5),
    };
    session_table.insert(encoded_session_id.clone(), current_session_data);

    // CREATE NEW COOKIE
    let cookie_values = CookieStruct {
        id: encoded_session_id,
        user_name: user_name.clone(),
    };
    let cookie = cookie::Cookie::build(COOKIE_NAME, json!(cookie_values).to_string())
        .path("/")
        .secure(false)
        .max_age(TimeDuration::minutes(30))
        .http_only(true)
        .finish();

    // RETURN AND ADD/REWRITE COOKIE
    HttpResponse::Ok().cookie(cookie).finish()
}

// LOGIN
pub async fn login(
    req_body: String,
    db_pool: web::Data<SqlitePool>,
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
                true => login_process(session_table_data, req, user_name),
                // WRONG PASSWORD
                false => HttpResponse::Ok().body(json!(format!(
                    "Password does not match the user: {}",
                    user_name
                ))),
            }
        }
        // USER NOT IN DB
        Err(_) => HttpResponse::Ok().body(json!(format!("User does not exist, please register."))),
    }
}

pub async fn logout(
    req: HttpRequest,
    session_table_data: web::Data<Mutex<HashMap<SessionID, SessionData>>>,
    // open_sockets_data: web::Data<Mutex<HashMap<SocketId, OpenSocketData>>>,
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

            // TODO REMOVE SOCKET FROM OPEN SOCKETS TO UPDATE OTHER USERS
            // let open_sockets_data_ref = open_sockets_data.get_ref();

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
