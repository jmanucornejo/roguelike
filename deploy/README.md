# Ubuntu server deployment

The production stack contains the headless game server, MySQL, and Caddy. Caddy
obtains and renews the HTTPS certificate used to deliver short-lived encrypted
Renet connection tokens. MySQL and the token issuer are not published directly.

## Prerequisites

- Ubuntu 24.04 LTS on an x86_64 server.
- Docker Engine and the Docker Compose plugin.
- A DNS `A` record such as `game.example.com` pointing to the server's public IPv4.
- Provider firewall inbound rules for TCP 22 (restricted to the administrator),
  TCP 80 and 443, UDP 443, and UDP 42069. Do not open TCP 3306 or TCP 8080.

## First deployment

```bash
git clone <repository-url> tribute
cd tribute
cp .env.production.example .env
openssl rand -hex 32
```

Put the generated value in `NETCODE_PRIVATE_KEY`, then set `GAME_DOMAIN`,
`SERVER_PUBLIC_ADDR`, and unique MySQL passwords in `.env`. The password inside
`DATABASE_URL` must match `MYSQL_PASSWORD`.

```bash
docker compose config
docker compose up -d --build
docker compose ps
docker compose logs -f server
```

The first image build compiles Rust and Bevy and can take several minutes.
Database data and Caddy certificates live in named Docker volumes and survive
container replacement.

The container sets `ASSET_ROOT=/app/assets`. Native deployments may point
`ASSET_ROOT` at an absolute assets directory; local repository runs detect the
workspace `assets/` directory automatically.

## Client package

Build the Windows client with the same snapshot feature as the server:

```powershell
cargo build --release --no-default-features --features client,batched_position_snapshots --bin client
```

Distribute `target/release/client.exe`, the `assets/` directory, and a
`client.env` copied from `client.env.example`. Set its URL to
`https://<GAME_DOMAIN>/token`. Never distribute `.env`, `DATABASE_URL`, or
`NETCODE_PRIVATE_KEY`.

## Updating

```bash
git pull --ff-only
docker compose up -d --build
docker compose logs --tail=100 server
```

## Local development

The local server and client default to unsecure loopback networking only. Start
the development database with:

```powershell
docker compose -f compose.dev.yaml up -d mysql phpmyadmin
cargo run --bin server
cargo run --bin client
```

For a local secure transport test, set a private key and use the internal token
endpoint directly:

```powershell
$env:NETCODE_PRIVATE_KEY="<64 hex characters>"
$env:SERVER_PUBLIC_ADDR="127.0.0.1:42069"
$env:SERVER_TOKEN_URL="http://127.0.0.1:8080/token"
```
