# marne Discord status bot

This Discord bot checks the status of your Battlefield marne servers.
You can use the bot with multiple methods, a config file, environment variable or within Docker.

### Environment items:

```yaml
token: discord bot token
game: (optional) game could be bf1 or bfv, defaults to bf1
server_name: servername to track
server_id: server id to track
set_banner_image: (optional) if it has to set the banner image of the bot (defaults to true)
```

## Using the bot

You can run it with Docker (Docker Compose):

```docker
version: '3.7'

services:
    ace-bot-1:
      image: ghcr.io/community-network/marne-bot/marne-bot:latest
      restart: always
      environment:
        - token=TOKEN
        - game=bf1
        - server_name=[ACE]#1
      healthcheck:
        test: ["CMD", "curl", "-f", "http://127.0.0.1:3030/"]
        interval: "60s"
        timeout: "3s"
        start_period: "5s"
        retries: 3
```

Or use the executable available [here](https://github.com/community-network/marne-bot/releases/latest)

And use that on windows via a bat file:

```bat
@ECHO OFF
SET token=DISCORDTOKEN
SET game=bf1
SET server_name=SUPER@ [SiC] S1
FILENAME.exe
```

Or on Linux/Mac with these commands:

```bash
export token=TOKEN
export game=bf1
export server_name=SERVERNAME
./FILENAME
```

or use the config.txt:

```yaml
# discord bot token
token = ''
# servername to track
server_name = ''
# (optional) game could be bf1 or bfv, defaults to bf1
game = 'bf1'
```

If you want to run it with your own changes in the code, install [rust](https://www.rust-lang.org/tools/install) and run with:

```bash
export token=TOKEN
export game=bf1
export server_name=SERVERNAME
cargo run
```
