# FOR FRONT END
FROM node:13.12.0-alpine
# SET WORKING DIR FOR WHOLE PROJECT
WORKDIR /usr/src/rusty_chat
ENV PATH /app/node_modules/.bin:$PATH

# install app dependencies
COPY chat_socket/package.json ./
COPY chat_socket/package-lock.json ./
RUN yarn install --silent

# move built files to the image file location
COPY ./chat_socket ./chat_socket/build

FROM rust:1.53.0
# will env be read here?
ENV DATABASE_URL=sqlite://./chat.db
COPY . .
RUN cargo install --path .

FROM debian:buster-slim
RUN apt-get update && apt-get install -y extra-runtime-dependencies && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/rusty_chat /usr/local/bin/rusty_chat
CMD ["rusty_chat"]