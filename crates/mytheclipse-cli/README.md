# mytheclipse-cli

CLI framework for mytheclipse applications with built-in subcommands:
`serve`, `worker`, `migrate`, `health`, and `version`.

## Features

| Feature | Default | Description |
| :--- | :---: | :--- |
| `clap-derive` | yes | Clap derive macros for argument parsing. |

## Usage

```toml
[dependencies]
mytheclipse-cli = "0.2"
```

```rust
use mytheclipse_cli::CliApp;

fn main() {
    let app = CliApp::parse();
    match app.command {
        Subcommand::Serve => { /* ... */ }
        Subcommand::Worker { topics } => { /* ... */ }
        Subcommand::Migrate => { /* ... */ }
        Subcommand::Health => { /* ... */ }
        Subcommand::Version => { println!("1.0.0"); }
    }
}
```
