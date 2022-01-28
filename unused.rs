// UNNEEDED CURRENTLY
// use actix_cors::Cors;
// use std::path::PathBuf;
// use fs::NamedFile;
// use sqlx::prelude;

/// Msg to signal a resend of information
// #[derive(Message)]
// #[rtype(result = "Result<bool, std::io::Error>")]
// struct ResendMessage;

// use actix_identity::{CookieIdentityPolicy, Identity, IdentityService, RequestIdentity};
// use std::fmt::{Debug, Formatter, Result};

// type ServerID = [u8; 32];


// NOTES: about://flags/ samesite flags

// UNUSED IMPORTS
// use actix_web::{http::header, middleware::Logger, App, HttpServer};
// use sqlx::FromRow;

// SAME SITE THINGS NOT USED
// .wrap(
//     // create identity middleware
//     IdentityService::new(
//         // create cookie identity policy
//         CookieIdentityPolicy::new(&[0; 32])
//             .name("auth-cookie")
//             .secure(true)
//             // NOT FOR
//             // .same_site(actix_web::cookie::SameSite::None),
//     ),
// )

// GENERATE A RANDOM 32 BYTE KEY.
// Note that it is important to use a unique
// private key for every project. Anyone with access to the key can generate
// authentication cookies for any user!
// let cookie_key = rand::thread_rng().gen::<[u8; 32]>();

// PW DEMO
// let password = b"password";
// let config = Config::default();
// let all_sockets = argon2::hash_encoded(password, SALT, &config).unwrap();
// let matches = argon2::verify_encoded(&all_sockets, password).unwrap();
// assert!(matches);

// CORS WRAPPING (NO LONGER NEEDED)
// .wrap(
//     Cors::default()
//         // .allowed_origin("http://localhost:3000")
//         .allowed_origin("http://127.0.0.1:8081/")
//         .allowed_methods(vec!["GET", "POST"])
//         .allowed_headers(vec![header::AUTHORIZATION, header::ACCEPT])
//         .allowed_header(header::CONTENT_TYPE)
//         .supports_credentials()
//         .max_age(3600),
// )

// A SINGLE STATIC FILE
// async fn static_files(req: HttpRequest) -> Result<NamedFile> {
//     println!("the req: {:?}", req);
//     let path: PathBuf = req
//         .match_info()
//         .query("./chat_socket/build")
//         .parse()
//         .unwrap();
//     println!("path: {:?}", path);
//     Ok(NamedFile::open(path)?)
// }

// .app_data(private_key.clone())

// generate some sort of private key here?
        // let private_key_hash: Vec<[u8; 32]> = vec![];
        // let private_key = web::Data::new(Mutex::new(private_key_hash));

// let sesh: Result<Option<SessionID>> = self.session.get(SESSION_ID_KEY);
        // let signed_in_user: Option<String>;

        // match sesh {
        //     Ok(s) => {
        //         if let Some(id) = s {
        //             // println!("Session id in stream: {}", id);
        //             signed_in_user = Some(id);
        //         } else {
        //             // println!("Result but no id in stream");
        //             signed_in_user = None;
        //         }
        //     }
        //     Err(_) => {
        //         println!("no result in stream");
        //         signed_in_user = None;
        //     }
        // }


// PAST CORS NOTES
// let cors = Cors::default()
//     .allowed_origin("http://localhost:3000/")
//     .supports_credentials()
//     .allowed_origin_fn(|origin, _req_head| origin.as_bytes().ends_with(b".rust-lang.org"))
//     .allowed_methods(vec!["GET", "POST"])
//     .allowed_headers(vec![http::header::AUTHORIZATION, http::header::ACCEPT])
//     // .allowed_header(http::header::CONTENT_TYPE)
//     .max_age(3600);

// HOT RELOADING WITH MONO-REPO
// use hotwatch::{Event, Hotwatch};
// use actix_files as fs;
// match env_dev {
//     Ok(_) => {
//         println!("in dev: hot reloading activated");
//         // HOT RELOADING
//         let mut hotwatch = Hotwatch::new().expect("hotwatch failed to initialize!");
//         hotwatch
//             .watch("./chat_socket/build", |event: Event| {
//                 if let Event::Write(_path) = event {
//                     println!("Changes in front-end");
//                 }
//             })
//             .expect("failed to watch file!");
//     }
//     Err(_) => {
//         println!("not in dev")
//     }
// }
