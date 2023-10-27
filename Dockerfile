FROM rust:1.67 as builder
WORKDIR /usr/src/myapp
COPY . .
ARG github_token 
RUN git config --global credential.helper store && echo "https://zefanjajobse:${github_token}@github.com" > ~/.git-credentials && cargo install --path .

FROM debian:bullseye-slim

ENV token default_token_value
ENV server_name default_server_name_value

HEALTHCHECK --interval=5m --timeout=3s --start-period=5s \
  CMD curl -f http://127.0.0.1:3030/ || exit 1

COPY --from=builder /usr/local/cargo/bin/discord_bot /usr/local/bin/discord_bot
RUN apt-get update && apt-get install --assume-yes curl
CMD echo "token = '$token'\nserver_name = '$server_name'" > config.txt && discord_bot
