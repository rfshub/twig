// src/modules/router/whitelist.rs

// This module provides a simple, constant list of paths that can bypass
// certain security checks like the blacklist and guard.
pub const WHITELISTED_PATHS: &[&str] = &["/"];
