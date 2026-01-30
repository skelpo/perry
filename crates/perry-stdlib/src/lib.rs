//! Standard Library for Perry
//!
//! Feature-gated implementations of Node.js APIs and npm packages.
//! Only compile what you actually use for smaller binaries.
//!
//! # Features
//! - `core` - Minimal runtime (always included)
//! - `http-server` - Native HTTP server (hyper-based)
//! - `http-client` - HTTP client (reqwest/node-fetch)
//! - `database` - All databases (postgres, mysql, sqlite, redis, mongodb)
//! - `crypto` - Cryptographic functions
//! - `compression` - zlib compression
//! - `full` - Everything (default)

// Core modules - always available
pub mod common;
pub mod dotenv;
pub mod slugify;
pub mod dayjs;
pub mod moment;
pub mod lodash;
pub mod events;
pub mod lru_cache;
pub mod commander;
pub mod decimal;
pub mod exponential_backoff;

// Re-export core
pub use common::*;
pub use dotenv::*;
pub use slugify::*;
pub use dayjs::*;
pub use moment::*;
pub use lodash::*;
pub use events::*;
pub use lru_cache::*;
pub use commander::*;
pub use decimal::*;
pub use exponential_backoff::*;

// === HTTP Server ===
#[cfg(feature = "http-server")]
pub mod framework;
#[cfg(feature = "http-server")]
pub use framework::*;

// === HTTP Client ===
#[cfg(feature = "http-client")]
pub mod fetch;
#[cfg(feature = "http-client")]
pub use fetch::*;

#[cfg(feature = "http-client")]
pub mod axios;
#[cfg(feature = "http-client")]
pub use axios::*;

// === WebSocket ===
#[cfg(feature = "websocket")]
pub mod ws;
#[cfg(feature = "websocket")]
pub use ws::*;

// === Databases ===
#[cfg(any(feature = "database-postgres", feature = "database-mysql"))]
pub mod pg;
#[cfg(any(feature = "database-postgres", feature = "database-mysql"))]
pub use pg::connection::*;
#[cfg(any(feature = "database-postgres", feature = "database-mysql"))]
pub use pg::pool::*;

#[cfg(any(feature = "database-postgres", feature = "database-mysql"))]
pub mod mysql2;
#[cfg(any(feature = "database-postgres", feature = "database-mysql"))]
pub use mysql2::connection::*;
#[cfg(any(feature = "database-postgres", feature = "database-mysql"))]
pub use mysql2::pool::*;

#[cfg(feature = "database-sqlite")]
pub mod sqlite;
#[cfg(feature = "database-sqlite")]
pub use sqlite::*;

#[cfg(feature = "database-redis")]
pub mod ioredis;
#[cfg(feature = "database-redis")]
pub use ioredis::*;

#[cfg(feature = "database-mongodb")]
pub mod mongodb;
#[cfg(feature = "database-mongodb")]
pub use mongodb::*;

// === Crypto ===
#[cfg(feature = "crypto")]
pub mod crypto;
#[cfg(feature = "crypto")]
pub use crypto::*;

// === Ethers (blockchain utilities) ===
#[cfg(feature = "crypto")]
pub mod ethers;
#[cfg(feature = "crypto")]
pub use ethers::*;

#[cfg(feature = "crypto")]
pub mod bcrypt;
#[cfg(feature = "crypto")]
pub use bcrypt::*;

#[cfg(feature = "crypto")]
pub mod argon2;
#[cfg(feature = "crypto")]
pub use argon2::*;

#[cfg(feature = "crypto")]
pub mod jsonwebtoken;
#[cfg(feature = "crypto")]
pub use jsonwebtoken::*;

// === Compression ===
#[cfg(feature = "compression")]
pub mod zlib;
#[cfg(feature = "compression")]
pub use zlib::*;

// === Email ===
#[cfg(feature = "email")]
pub mod nodemailer;
#[cfg(feature = "email")]
pub use nodemailer::*;

// === Image Processing ===
#[cfg(feature = "image")]
pub mod sharp;
#[cfg(feature = "image")]
pub use sharp::*;

// === HTML Parsing ===
#[cfg(feature = "html-parser")]
pub mod cheerio;
#[cfg(feature = "html-parser")]
pub use cheerio::*;

// === Scheduler ===
#[cfg(feature = "scheduler")]
pub mod cron;
#[cfg(feature = "scheduler")]
pub use cron::*;

// === Rate Limiting ===
#[cfg(feature = "rate-limit")]
pub mod ratelimit;
#[cfg(feature = "rate-limit")]
pub use ratelimit::*;

// === Validation ===
#[cfg(feature = "validation")]
pub mod validator;
#[cfg(feature = "validation")]
pub use validator::*;

// === IDs ===
#[cfg(feature = "ids")]
pub mod uuid;
#[cfg(feature = "ids")]
pub use uuid::*;

#[cfg(feature = "ids")]
pub mod nanoid;
#[cfg(feature = "ids")]
pub use nanoid::*;
