# rusty_chat
A high-performance live chat server using actix-web's actor framework.


### set up from greenfield
`docker run --name rusty-chat-postgres -e POSTGRES_PASSWORD=mysecretpassword -dp 5432:5432 postgres`
- ensure you have postgres installed on your machine (diesel-cli requires)
- install diesel-cli `cargo install diesel_cli --no-default-features --features postgres`
- `diesel setup` to set up database
- run `diesel migration run` while in the repo to build database
- start/compile the server with `cargo run`

### next steps
- move to postgresql + diesel from sqlite
- set updates to be a better system
  - initial update
  - incremental updates straight to user
### future updates
  - set up redis caching system between application and database for more efficient updates and less database interactions