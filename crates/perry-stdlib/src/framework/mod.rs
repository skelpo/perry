//! Perry Native Framework
//!
//! A high-performance HTTP/WebSocket server framework optimized for native compilation.
//! Inspired by Hono's functional middleware pattern.
//!
//! # Example
//! ```typescript
//! import { serve, router } from 'perry/http';
//!
//! const app = router()
//!   .get('/', (c) => c.text('Hello World'))
//!   .post('/users', async (c) => {
//!     const body = await c.json();
//!     return c.json({ created: true }, 201);
//!   });
//!
//! serve(app, { port: 3000 });
//! ```

pub mod server;
pub mod request;
pub mod response;
pub mod json;

// Re-export main types
pub use server::*;
pub use request::*;
pub use response::*;
pub use json::*;
