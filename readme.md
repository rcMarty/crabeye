# Ranal

**Ranal** is a simple, efficient tool for analyzing GitHub repositories primarly Rust repository.
Track everything about contributors, issues, and repository activity — all written in Rust.

## Features (Planned)

- Analyze contributor activity.
- Track open and closed issues.
- Gather repository statistics like commits, pull requests, and more.

## Database

## Usage

First of all, you need to setup your postgres database.

*in .idea there are all configs for database `ranal` on `localhost`*

*if you dont have sqlx-cli installed, you can do it by
running: ``cargo install sqlx-cli --features native-tls, sqlite``*

And then you can run migrations:

```bash
sqlx migrate run
```

also you must have

When you want to get data for analyzing (so far only obtaining data to database), you need to add subcommand `analyze`
to the command.

```bash
cargo run --release -- analyze
```

Otherwise, if you want to only try some analytics queries, you can find some examples under subcommand `request`.

```bash
cargo run --release -- request help
```

Link to database schema [here](https://dbdiagram.io/d/6791134c37f5d6cbeb969453)

## TODO

- add tests

## License

This project is licensed under the MIT License. See the LICENSE file for details.