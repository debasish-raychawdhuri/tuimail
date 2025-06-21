pub mod app;
pub mod config;
pub mod credentials;
pub mod database;
pub mod email;
pub mod ui;
pub mod spellcheck;
pub mod grammarcheck;
pub mod async_grammar;

// Re-export commonly used types
pub use app::App;
pub use config::{Config, EmailAccount};
pub use database::EmailDatabase;
pub use email::{Email, EmailClient, EmailError};
