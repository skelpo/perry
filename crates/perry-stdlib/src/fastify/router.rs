//! Route pattern parsing and matching
//!
//! Supports:
//! - Static segments: `/users`, `/api/v1`
//! - Parameters: `/users/:id`, `/posts/:postId/comments/:commentId`
//! - Wildcards: `/static/*` (captures rest of path)

use std::collections::HashMap;

/// A segment in a route pattern
#[derive(Debug, Clone, PartialEq)]
pub enum Segment {
    /// Static path segment (e.g., "users")
    Static(String),
    /// Named parameter (e.g., ":id" captures "id")
    Param(String),
    /// Wildcard captures rest of path
    Wildcard,
}

/// Parsed route pattern for efficient matching
#[derive(Debug, Clone)]
pub struct RoutePattern {
    /// Pattern segments
    pub segments: Vec<Segment>,
    /// Original pattern string
    pub raw: String,
}

impl RoutePattern {
    /// Parse a route pattern string into segments
    ///
    /// # Examples
    /// - `/users` -> [Static("users")]
    /// - `/users/:id` -> [Static("users"), Param("id")]
    /// - `/static/*` -> [Static("static"), Wildcard]
    pub fn parse(path: &str) -> Self {
        let mut segments = Vec::new();
        let path = path.trim_start_matches('/');

        if path.is_empty() {
            return Self {
                segments,
                raw: "/".to_string(),
            };
        }

        for part in path.split('/') {
            if part.is_empty() {
                continue;
            }

            let segment = if part.starts_with(':') {
                // Parameter segment
                Segment::Param(part[1..].to_string())
            } else if part == "*" {
                // Wildcard segment
                Segment::Wildcard
            } else {
                // Static segment
                Segment::Static(part.to_string())
            };

            segments.push(segment);
        }

        Self {
            segments,
            raw: path.to_string(),
        }
    }

    /// Match a request path against this pattern
    ///
    /// Returns `Some(params)` if the path matches, with extracted parameters.
    /// Returns `None` if the path doesn't match.
    pub fn match_path(&self, path: &str) -> Option<HashMap<String, String>> {
        let path = path.trim_start_matches('/');
        let path = path.split('?').next().unwrap_or(path); // Remove query string
        let path_parts: Vec<&str> = if path.is_empty() {
            Vec::new()
        } else {
            path.split('/').filter(|s| !s.is_empty()).collect()
        };

        // Handle root path
        if self.segments.is_empty() {
            return if path_parts.is_empty() {
                Some(HashMap::new())
            } else {
                None
            };
        }

        let mut params = HashMap::new();
        let mut path_idx = 0;

        for (seg_idx, segment) in self.segments.iter().enumerate() {
            match segment {
                Segment::Static(expected) => {
                    if path_idx >= path_parts.len() || path_parts[path_idx] != expected {
                        return None;
                    }
                    path_idx += 1;
                }
                Segment::Param(name) => {
                    if path_idx >= path_parts.len() {
                        return None;
                    }
                    params.insert(name.clone(), path_parts[path_idx].to_string());
                    path_idx += 1;
                }
                Segment::Wildcard => {
                    // Wildcard captures the rest of the path
                    let rest: String = path_parts[path_idx..].join("/");
                    params.insert("*".to_string(), rest);
                    return Some(params); // Wildcard always matches rest
                }
            }
        }

        // All segments matched, check if we consumed all path parts
        if path_idx == path_parts.len() {
            Some(params)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_route() {
        let pattern = RoutePattern::parse("/users");
        assert!(pattern.match_path("/users").is_some());
        assert!(pattern.match_path("/users/").is_some());
        assert!(pattern.match_path("/posts").is_none());
        assert!(pattern.match_path("/users/123").is_none());
    }

    #[test]
    fn test_param_route() {
        let pattern = RoutePattern::parse("/users/:id");

        let params = pattern.match_path("/users/123").unwrap();
        assert_eq!(params.get("id"), Some(&"123".to_string()));

        let params = pattern.match_path("/users/abc").unwrap();
        assert_eq!(params.get("id"), Some(&"abc".to_string()));

        assert!(pattern.match_path("/users").is_none());
        assert!(pattern.match_path("/users/123/posts").is_none());
    }

    #[test]
    fn test_multiple_params() {
        let pattern = RoutePattern::parse("/posts/:postId/comments/:commentId");

        let params = pattern.match_path("/posts/1/comments/2").unwrap();
        assert_eq!(params.get("postId"), Some(&"1".to_string()));
        assert_eq!(params.get("commentId"), Some(&"2".to_string()));
    }

    #[test]
    fn test_wildcard() {
        let pattern = RoutePattern::parse("/static/*");

        let params = pattern.match_path("/static/css/style.css").unwrap();
        assert_eq!(params.get("*"), Some(&"css/style.css".to_string()));

        let params = pattern.match_path("/static/").unwrap();
        assert_eq!(params.get("*"), Some(&"".to_string()));
    }

    #[test]
    fn test_root_route() {
        let pattern = RoutePattern::parse("/");
        assert!(pattern.match_path("/").is_some());
        assert!(pattern.match_path("").is_some());
        assert!(pattern.match_path("/users").is_none());
    }

    #[test]
    fn test_query_string_ignored() {
        let pattern = RoutePattern::parse("/users/:id");
        let params = pattern.match_path("/users/123?foo=bar").unwrap();
        assert_eq!(params.get("id"), Some(&"123".to_string()));
    }

    #[test]
    fn test_nested_params() {
        let pattern = RoutePattern::parse("/api/v1/users/:userId/posts/:postId");
        let params = pattern.match_path("/api/v1/users/42/posts/99").unwrap();
        assert_eq!(params.get("userId"), Some(&"42".to_string()));
        assert_eq!(params.get("postId"), Some(&"99".to_string()));
    }

    #[test]
    fn test_mixed_static_and_params() {
        let pattern = RoutePattern::parse("/users/:id/profile");
        assert!(pattern.match_path("/users/123/profile").is_some());
        assert!(pattern.match_path("/users/123/settings").is_none());
        assert!(pattern.match_path("/users/123").is_none());
    }
}

#[cfg(test)]
mod app_tests {
    use crate::fastify::FastifyApp;

    #[test]
    fn test_add_routes() {
        let mut app = FastifyApp::new();

        // Add some routes
        app.add_route("GET", "/", 0);
        app.add_route("GET", "/users", 1);
        app.add_route("GET", "/users/:id", 2);
        app.add_route("POST", "/users", 3);

        assert_eq!(app.routes.len(), 4);
    }

    #[test]
    fn test_route_matching() {
        let mut app = FastifyApp::new();

        app.add_route("GET", "/", 100);
        app.add_route("GET", "/users", 101);
        app.add_route("GET", "/users/:id", 102);
        app.add_route("POST", "/users", 103);
        app.add_route("PUT", "/users/:id", 104);

        // Test root
        let (route, params) = app.match_route("GET", "/").unwrap();
        assert_eq!(route.handler, 100);
        assert!(params.is_empty());

        // Test static
        let (route, params) = app.match_route("GET", "/users").unwrap();
        assert_eq!(route.handler, 101);
        assert!(params.is_empty());

        // Test param
        let (route, params) = app.match_route("GET", "/users/42").unwrap();
        assert_eq!(route.handler, 102);
        assert_eq!(params.get("id"), Some(&"42".to_string()));

        // Test POST
        let (route, _) = app.match_route("POST", "/users").unwrap();
        assert_eq!(route.handler, 103);

        // Test PUT with param
        let (route, params) = app.match_route("PUT", "/users/99").unwrap();
        assert_eq!(route.handler, 104);
        assert_eq!(params.get("id"), Some(&"99".to_string()));

        // Test 404 cases
        assert!(app.match_route("DELETE", "/users").is_none());
        assert!(app.match_route("GET", "/posts").is_none());
    }

    #[test]
    fn test_hooks() {
        let mut app = FastifyApp::new();

        app.add_hook("onRequest", 1);
        app.add_hook("preHandler", 2);
        app.add_hook("preHandler", 3);

        assert_eq!(app.hooks.on_request.len(), 1);
        assert_eq!(app.hooks.pre_handler.len(), 2);
    }

    #[test]
    fn test_error_handler() {
        let mut app = FastifyApp::new();
        assert!(app.error_handler.is_none());

        app.set_error_handler(42);
        assert_eq!(app.error_handler, Some(42));
    }

    #[test]
    fn test_plugin_prefix() {
        let scoped = FastifyApp::with_prefix("/api/v1".to_string());
        assert_eq!(scoped.prefix, "/api/v1");
    }

    #[test]
    fn test_route_with_prefix() {
        let mut app = FastifyApp::with_prefix("/api".to_string());
        app.add_route("GET", "/users", 1);

        // Route should be /api/users
        let (route, _) = app.match_route("GET", "/api/users").unwrap();
        assert_eq!(route.handler, 1);

        // Plain /users should not match
        assert!(app.match_route("GET", "/users").is_none());
    }
}
