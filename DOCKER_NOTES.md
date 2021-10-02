tried to dockerize with basic command, got this
```
#8 128.7    Compiling actix-web-actors v3.0.0
#8 129.0    Compiling actix-session v0.4.1
#8 130.4    Compiling rusty_chat v0.1.0 (/usr/src/myapp)
#8 130.6 error: `DATABASE_URL` must be set to use query macros
#8 130.6   --> src/auth.rs:46:16
#8 130.6    |
#8 130.6 46 |     let user = sqlx::query!(r#"SELECT id FROM user WHERE name=$1"#, user_name)
#8 130.6    |                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
#8 130.6    |
#8 130.6    = note: this error originates in the macro `$crate::sqlx_macros::expand_query` (in Nightly builds, run with -Z macro-backtrace for more info)
#8 130.6 
#8 130.6 error: `DATABASE_URL` must be set to use query macros
#8 130.6   --> src/auth.rs:60:13
#8 130.6    |
#8 130.6 60 | /             sqlx::query!(
#8 130.6 61 | |                 r#"INSERT INTO user (name, hash, salt) VALUES ($1, $2, $3)"#,
#8 130.6 62 | |                 user_name,
#8 130.6 63 | |                 password_hash,
#8 130.6 64 | |                 salt
#8 130.6 65 | |             )
#8 130.6    | |_____________^
#8 130.6    |
#8 130.6    = note: this error originates in the macro `$crate::sqlx_macros::expand_query` (in Nightly builds, run with -Z macro-backtrace for more info)
#8 130.6 
#8 130.6 error: `DATABASE_URL` must be set to use query macros
#8 130.6    --> src/auth.rs:144:11
#8 130.6     |
#8 130.6 144 |     match sqlx::query!(r#"SELECT * FROM user WHERE name=$1"#, user_name)
#8 130.6     |           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
#8 130.6     |
#8 130.6     = note: this error originates in the macro `$crate::sqlx_macros::expand_query` (in Nightly builds, run with -Z macro-backtrace for more info)
#8 130.6 
#8 130.7 error: `DATABASE_URL` must be set to use query macros
#8 130.7    --> src/socket_actor.rs:272:26
#8 130.7     |
#8 130.7 272 |     let user_id_record = sqlx::query!(r#"SELECT id FROM user WHERE name=$1"#, id)
#8 130.7     |                          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
#8 130.7     |
#8 130.7     = note: this error originates in the macro `$crate::sqlx_macros::expand_query` (in Nightly builds, run with -Z macro-backtrace for more info)
#8 130.7 
#8 130.7 error: `DATABASE_URL` must be set to use query macros
#8 130.7    --> src/socket_actor.rs:277:26
#8 130.7     |
#8 130.7 277 |     let room_id_record = sqlx::query!(r#"SELECT id FROM room WHERE name=$1"#, default_room)
#8 130.7     |                          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
#8 130.7     |
#8 130.7     = note: this error originates in the macro `$crate::sqlx_macros::expand_query` (in Nightly builds, run with -Z macro-backtrace for more info)
#8 130.7 
#8 130.7 error: `DATABASE_URL` must be set to use query macros
#8 130.7    --> src/socket_actor.rs:291:5
#8 130.7     |
#8 130.7 291 | /     sqlx::query!(
#8 130.7 292 | |         r#"INSERT INTO message (user_id, room_id, message, time) VALUES ($1, $2, $3, $4)"#,
#8 130.7 293 | |         message_to_db.user_id,
#8 130.7 294 | |         message_to_db.room_id,
#8 130.7 295 | |         message_to_db.message,
#8 130.7 296 | |         current_time
#8 130.7 297 | |     )
#8 130.7     | |_____^
#8 130.7     |
#8 130.7     = note: this error originates in the macro `$crate::sqlx_macros::expand_query` (in Nightly builds, run with -Z macro-backtrace for more info)
#8 130.7 
#8 130.7 error: `DATABASE_URL` must be set to use query macros
#8 130.7    --> src/socket_actor.rs:328:7
#8 130.7     |
#8 130.7 328 |       sqlx::query_as!(DatabaseMessage, "SELECT message.id, user_id, room_id, message, time, name FROM message INNER JOIN user on user.id=message.user_id ORDER BY time DESC LIMIT 50")
#8 130.7     |       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
#8 130.7     |
#8 130.7     = note: this error originates in the macro `$crate::sqlx_macros::expand_query` (in Nightly builds, run with -Z macro-backtrace for more info)
#8 130.7 
#8 130.9 error: could not compile `rusty_chat` due to 7 previous errors
#8 130.9 warning: build failed, waiting for other jobs to finish...
#8 137.8 error: failed to compile `rusty_chat v0.1.0 (/usr/src/myapp)`, intermediate artifacts can be found at `/usr/src/myapp/target`
#8 137.8 
#8 137.8 Caused by:
#8 137.8   build failed
------
executor failed running [/bin/sh -c cargo install --path .]: exit code: 101
    ~/rust_projects/chat  on   master !1 ─────────────────────────────────── took 4m 7s   at 16:46:06  
❯ 
```