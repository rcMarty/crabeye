# Ranal

**Ranal** is a simple, efficient tool for analyzing GitHub repositories primarly Rust repository.
Track everything about contributors, issues, and repository activity — all written in Rust.

## Features (Planned)

- Analyze contributor activity.
- Track open and closed issues.
- Gather repository statistics like commits, pull requests, and more.

## Database

### Setup via Docker

You can use [docker compose](../docker-compose.yml) to run a postgres database.
That database will have ready all migrations that you can find under `migrations` folder.
You can also run postgres database locally, but you need to create database manually.

### Setup via local Postgres

Make sure you have a running Postgres instance.
Create a database named `ranal` (or any name you prefer) and set up the connection string in the
`.env` file.

Now you can run migrations using `sqlx-cli`.

```bash
cd backend
sqlx migrate run
```

*if you dont have sqlx-cli installed, you can do it by
running: ``cargo install sqlx-cli --features native-tls``*

## Configuration

Create a `.env` file in the `backend` directory from the provided `.env.example` file and fill in
the required environment variables.

## Usage

This command will fill the database with last 500 pull requests as demonstration.

```bash
cargo run -- analyze --sync 5
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