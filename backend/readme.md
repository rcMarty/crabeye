# Crabeye

**Crabeye** is a simple, efficient tool for analyzing GitHub repositories primarly Rust repository.
Track everything about contributors, issues, and repository activity — all written in Rust.

## Features (Planned)

- Analyze contributor activity.
- Track open and closed issues.
- Gather repository statistics like commits, pull requests, and more.

## Crate layout

`crabeye` now builds both as:

- a CLI application (default build), and
- a library crate with feature-gated modules.

### Library features

- `db` — database models and query layer
- `git` — Git + GitHub synchronization (`db` is enabled automatically)
- `api` — REST API routes (`db` is enabled automatically)
- `monitoring` — periodic sync monitoring (`git` is enabled automatically)
- `cli` — console application entrypoint and runtime setup (enabled by default)

### Feature aliases

- `sync` → `git`
- `server` → `api` + `monitoring`
- `full` → `cli`

Examples:

```bash
cargo build
cargo build --lib --no-default-features --features db
cargo build --lib --no-default-features --features git
cargo build --lib --no-default-features --features api
cargo build --lib --no-default-features --features sync
cargo build --lib --no-default-features --features server
cargo build --no-default-features --features full
```

## Configuration

Create a `.env` file in the `backend` directory from the provided `.env.example` file and fill in
the required environment variables.

## Database

### Setup via Docker

You can use [docker compose](./docker-compose.yml) to run a postgres database.
You can also run postgres database locally.

```bash
docker compose up -d
sqlx migrate run
```

### Setup via local Postgres

Make sure you have a running Postgres instance.
Create a database named `crabeye` (or any name you prefer) and set up the connection string in the
`.env` file.

Now you can create database `sqlx database create` and run migrations using `sqlx migrate run`.
*database will be created as is in .nev file*

*if you dont have sqlx-cli installed, you can do it by
running: ``cargo install sqlx-cli --features native-tls``*

## Usage

This command will fill the database with the last 500 pull requests as a demonstration. *(The number
is the number of pages to fetch; each page contains up to 100 items.)*

```bash
cargo run -- sync-all --sync 5
```

If you have all the data you want you can run the server:

```bash
cargo run -- serve
```

Of course, you can always run command or subcommand with `--help` to see all available options.

Then you can access the API docs at `http://localhost:7878/docs`.

[//]: # (Link to database schema [here]&#40;https://dbdiagram.io/d/6791134c37f5d6cbeb969453&#41; TODO add link when ready)

## TODO

- add tests

## License

This project is licensed under the MIT License. See the LICENSE file for details.