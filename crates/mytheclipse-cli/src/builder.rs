//! Clap-based CLI builder implementation.

use clap::{Parser, Subcommand as ClapSubcommand};

/// A mytheclipse CLI application.
#[derive(Parser, Debug)]
#[command(name = "myapp", version, about)]
pub struct CliApp {
    #[command(subcommand)]
    pub command: Subcommand,
}

/// Built-in subcommands for mytheclipse applications.
#[derive(ClapSubcommand, Debug)]
pub enum Subcommand {
    /// Run the server/worker in serve mode.
    Serve,
    /// Run background job workers.
    Worker {
        /// Topic(s) to consume from.
        topics: Vec<String>,
    },
    /// Run database migrations.
    Migrate,
    /// Check service health.
    Health,
    /// Print version information.
    Version,
}

/// Builder for CliApp with configuration.
pub struct CliBuilder {
    name: String,
    about: String,
}

impl Default for CliBuilder {
    fn default() -> Self {
        Self {
            name: "myapp".to_string(),
            about: "A mytheclipse application".to_string(),
        }
    }
}

impl CliBuilder {
    pub fn new(name: impl Into<String>, about: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            about: about.into(),
        }
    }

    pub fn build(self) -> CliApp {
        CliApp::parse()
    }
}
