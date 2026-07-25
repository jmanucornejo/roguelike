# Running

```powershell
cargo run --bin server
cargo run --bin client
```

# Optional client features

```powershell
# Local movement prediction
cargo run --bin client --features client_prediction
```

# Optional MySQL persistence

The server keeps its current in-memory behavior when `DATABASE_URL` is not set.
For local development, start the included MySQL container:

```powershell
docker-compose up -d mysql
docker-compose ps
```

The database files are stored in `docker/mysql/` inside the project and are
ignored by Git. Running `docker-compose down` stops and removes the container but
keeps that database data.

If Docker exposes Compose as a CLI plugin on your system, the equivalent command
uses `docker compose` (without the hyphen).

Copy `.env.example` to `.env`, or set the connection URL in the shell:

```powershell
$env:DATABASE_URL="mysql://roguelike:change-me@127.0.0.1:3306/roguelike"
cargo run --bin server
```

The server applies the versioned migrations in `migrations/` when it connects.
The Compose service provides the dedicated `roguelike` user with the development
password `change-me`; do not reuse these credentials in production.

## Development character persistence

Until login and character selection are implemented, `PLAYER_ACCOUNT_ID` in
`.env` identifies the local development account. The server automatically creates
slot 0 the first time that account connects, then loads the same character and its
saved position, facing, HP, SP, levels, experience, and zeny on later connections.

To run a second client at the same time, give its process a different account id:

```powershell
$env:PLAYER_ACCOUNT_ID="2"
cargo run --bin client
```
