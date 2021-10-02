FROM rust:1.55

ENV DATABASE_URL=sqlite://./chat.db
ENV LOCAL_URL=127.0.0.1:8081

WORKDIR /usr/src/myapp

COPY . .

RUN cargo install --path .

CMD ["rusty_chat"]