FROM rust:buster as builder
WORKDIR /app

ENV DATABASE_URL=sqlite://./chat.db

COPY . .

RUN cargo build --release

FROM bitnami/minideb:buster as runner
WORKDIR /app

ENV DATABASE_URL=sqlite://./chat.db

COPY --from=builder /app/target/release/rusty_chat rusty_chat
COPY --from=builder /app/chat.db chat.db

CMD /app/rusty_chat