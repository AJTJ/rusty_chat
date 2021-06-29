FROM rust:1.53.0

WORKDIR /usr/src/rusty_chat
COPY . .
RUN cargo install --path .
CMD ["rusty_chat"]