# Ranal

**Ranal** is a simple, efficient tool for analyzing GitHub repositories primarly Rust repository.
Track everything about contributors, issues, and repository activity — all written in Rust.

## Features (Planned)

- Analyze contributor activity.
- Track open and closed issues.
- Gather repository statistics like commits, pull requests, and more.

## Database

## Usage

First of all, you need to setup your database. You can do it by running the following command:

*if you dont have sqlx-cli installed, you can do it by
running: ``cargo install sqlx-cli --features native-tls, sqlite``*

```bash
sqlx database create
sqlx migrate run
```

and also you must have

When you want to get data for analyzing, you need to add argument `--analyze` to the command.

```bash
cargo run --release -- --analyze
```

Otherwise, if you want to only try analysis and so on you can run it without arguments

```bash
cargo run --release
```

Link to database schema [here](https://dbdiagram.io/d/6791134c37f5d6cbeb969453)

## TODO

- [ ] ratatui TUI
- [ ] 

## License

This project is licensed under the MIT License. See the LICENSE file for details.