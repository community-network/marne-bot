FROM rust:1.75 AS builder
WORKDIR /usr/src/myapp
COPY . .
ARG github_token
ARG CARGO_NET_GIT_FETCH_WITH_CLI=true
RUN git config --global credential.helper store && echo "https://zefanjajobse:${github_token}@github.com" > ~/.git-credentials && cargo install --path .

FROM debian:bookworm-slim

ENV token default_token_value
ENV game bf1
ENV server_name default_server_name_value
ENV server_id default_server_id_value

HEALTHCHECK --interval=5m --timeout=3s --start-period=5s \
  CMD curl -f http://127.0.0.1:3030/ || exit 1

COPY --from=builder /usr/local/cargo/bin/discord_bot /usr/local/bin/discord_bot
RUN apt-get update && apt-get install --assume-yes curl && apt-get clean
CMD if [ "$server_name" = "default_server_name_value" ]; then\
  export INFO="token = '$token'\ngame = '$game'\nserver_id = $server_id"; \
  else \
  export INFO="token = '$token'\ngame = '$game'\nserver_name = '$server_name'"; \
  fi; \
  echo $INFO > config.txt && discord_bot