# build and run with: 
# docker build -t rusty_chat . && docker run -it --rm --name rusty_chat_running rusty_chat

# FOR FRONT END
FROM node:15.0.1
# SET WORKING DIR FOR WHOLE PROJECT
WORKDIR /usr/src/rusty_chat

# IS THIS NEEDED FOR NODE?
# ENV PATH /app/node_modules/.bin:$PATH

# install app dependencies into a chat_socket dir
RUN mkdir chat_socket
COPY chat_socket/package.json ./
COPY chat_socket/yarn.lock ./
RUN cd chat_socket && yarn install && yarn build

# HOW TO DELETE EVERYTHING IN THE REACT APP EXCEPT THE BUILD FOLDER?

# # RUST DOCKER SETUP
# # https://hub.docker.com/_/rust
# FROM rust:1.53.0
# # SET ENV HERE OR SET IT ABOVE WITH OTHER ENV?
# ENV DATABASE_URL=sqlite://./chat.db
# # COPY ALL TO WORKING DIR
# COPY . .
# # INSTALL PROJECT
# RUN cargo install --path .

# # 
# FROM debian:buster-slim
# RUN apt-get update && apt-get install -y extra-runtime-dependencies && rm -rf /var/lib/apt/lists/*
# COPY --from=builder /usr/local/cargo/bin/rusty_chat /usr/local/bin/rusty_chat
# # START THE SERVER
# CMD ["rusty_chat"]