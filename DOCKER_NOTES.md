to build
`docker build -t theloniousjanke/rusty_chat .`
to run first time
`docker run --name rusty_chat -dp 8081:8081 theloniousjanke/rusty_chat`
to run other times
`docker run -dp 8081:8081 theloniousjanke/rusty_chat`

https://ajtj.github.io/chat_frontend/