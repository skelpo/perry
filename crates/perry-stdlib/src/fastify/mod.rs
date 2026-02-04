//! Fastify-Compatible Native HTTP Framework for Perry
//!
//! A high-performance HTTP framework with Fastify-like API and Hono-style context methods.
//! Compiles TypeScript to native code while providing familiar patterns.
//!
//! # Example (Fastify style)
//! ```typescript
//! import Fastify from 'fastify';
//!
//! const app = Fastify();
//!
//! app.get('/', async (request, reply) => {
//!   return { hello: 'world' };
//! });
//!
//! app.listen({ port: 3000 });
//! ```
//!
//! # Example (Hono style)
//! ```typescript
//! app.get('/users/:id', async (c) => {
//!   return c.json({ id: c.req.param('id') });
//! });
//! ```

pub mod router;
pub mod context;
pub mod app;
pub mod server;

pub use router::*;
pub use context::*;
pub use app::*;
pub use server::*;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Closure pointer type (matches perry-runtime)
pub type ClosurePtr = i64;

/// Route definition
#[derive(Clone)]
pub struct Route {
    /// HTTP method (GET, POST, etc.)
    pub method: String,
    /// Route pattern with parameter extraction
    pub pattern: RoutePattern,
    /// Handler closure pointer
    pub handler: ClosurePtr,
}

/// Lifecycle hooks for request processing
#[derive(Default, Clone)]
pub struct Hooks {
    /// Called when a request is received
    pub on_request: Vec<ClosurePtr>,
    /// Called before body parsing
    pub pre_parsing: Vec<ClosurePtr>,
    /// Called before validation
    pub pre_validation: Vec<ClosurePtr>,
    /// Called before the route handler
    pub pre_handler: Vec<ClosurePtr>,
    /// Called before serialization
    pub pre_serialization: Vec<ClosurePtr>,
    /// Called before sending response
    pub on_send: Vec<ClosurePtr>,
    /// Called after response is sent
    pub on_response: Vec<ClosurePtr>,
    /// Called when an error occurs
    pub on_error: Vec<ClosurePtr>,
}

/// Plugin registration
#[derive(Clone)]
pub struct Plugin {
    /// Plugin handler closure
    pub handler: ClosurePtr,
    /// URL prefix for all routes in this plugin
    pub prefix: String,
}

/// Main Fastify application instance
pub struct FastifyApp {
    /// Registered routes
    pub routes: Vec<Route>,
    /// Lifecycle hooks
    pub hooks: Hooks,
    /// Custom error handler
    pub error_handler: Option<ClosurePtr>,
    /// Registered plugins
    pub plugins: Vec<Plugin>,
    /// Route prefix (for plugins)
    pub prefix: String,
    /// Server configuration
    pub config: FastifyConfig,
}

/// Server configuration options
#[derive(Clone, Default)]
pub struct FastifyConfig {
    /// Enable request logging
    pub logger: bool,
    /// Trust proxy headers
    pub trust_proxy: bool,
    /// Maximum body size in bytes
    pub body_limit: Option<usize>,
}

impl FastifyApp {
    /// Create a new Fastify application
    pub fn new() -> Self {
        Self {
            routes: Vec::new(),
            hooks: Hooks::default(),
            error_handler: None,
            plugins: Vec::new(),
            prefix: String::new(),
            config: FastifyConfig::default(),
        }
    }

    /// Create a new Fastify application with a prefix (for plugins)
    pub fn with_prefix(prefix: String) -> Self {
        Self {
            routes: Vec::new(),
            hooks: Hooks::default(),
            error_handler: None,
            plugins: Vec::new(),
            prefix,
            config: FastifyConfig::default(),
        }
    }

    /// Add a route
    pub fn add_route(&mut self, method: &str, path: &str, handler: ClosurePtr) {
        let full_path = if self.prefix.is_empty() {
            path.to_string()
        } else {
            format!("{}{}", self.prefix, path)
        };

        self.routes.push(Route {
            method: method.to_uppercase(),
            pattern: RoutePattern::parse(&full_path),
            handler,
        });
    }

    /// Add a hook
    pub fn add_hook(&mut self, hook_name: &str, handler: ClosurePtr) {
        match hook_name {
            "onRequest" => self.hooks.on_request.push(handler),
            "preParsing" => self.hooks.pre_parsing.push(handler),
            "preValidation" => self.hooks.pre_validation.push(handler),
            "preHandler" => self.hooks.pre_handler.push(handler),
            "preSerialization" => self.hooks.pre_serialization.push(handler),
            "onSend" => self.hooks.on_send.push(handler),
            "onResponse" => self.hooks.on_response.push(handler),
            "onError" => self.hooks.on_error.push(handler),
            _ => eprintln!("Unknown hook: {}", hook_name),
        }
    }

    /// Set error handler
    pub fn set_error_handler(&mut self, handler: ClosurePtr) {
        self.error_handler = Some(handler);
    }

    /// Find matching route for a request
    pub fn match_route(&self, method: &str, path: &str) -> Option<(&Route, HashMap<String, String>)> {
        for route in &self.routes {
            if route.method == method {
                if let Some(params) = route.pattern.match_path(path) {
                    return Some((route, params));
                }
            }
        }
        None
    }
}

impl Default for FastifyApp {
    fn default() -> Self {
        Self::new()
    }
}
