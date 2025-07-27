// src/modules/router/blacklist.rs

use crate::core::response;
use crate::modules::router::whitelist;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use rand::{seq::SliceRandom, Rng};

const RESP_418_PATHS: &[&str] = &[
    "/wp-login.php",
    "/wp-admin",
    "/wp-admin/setup-config.php",
    "/wordpress/wp-admin/setup-config.php",
    "/wp-content/",
    "/wp-includes/",
    "/wp-json/",
    "/xmlrpc.php",
    "/wp-config.php",
    "/wp-config.php.bak",
    "/wp-config.php.old",
    "/wp-config.php.save",
    "/wp-config-sample.php",
    "/wp-cron.php",
    "/wp-mail.php",
    "/wp-trackback.php",
    "/readme.html",
    "/license.txt",
    "/wp-activate.php",
    "/wp-comments-post.php",
    "/wp-links-opml.php",
    "/wp-load.php",
    "/wp-settings.php",
    "/wp-signup.php",
    "/wp-blog-header.php",
    "/plugins/",
    "/themes/",
    "/uploads/",
    "/phpinfo.php",
    "/phpmyadmin/",
];

const RESP_403_PATHS: &[&str] = &[
    "/admin",
    "/admin/",
    "/admin/login",
    "/admin.php",
    "/admin.html",
    "/administrator",
    "/administrator/",
    "/admin-login",
    "/login",
    "/logon",
    "/login.php",
    "/login.html",
    "/register",
    "/signup",
    "/dashboard",
    "/.env",
    "/.git/config",
    "/.git",
    "/.svn",
    "/.htaccess",
    "/.idea",
    "/.vscode",
    "/.gitignore",
];

const RESP_400_PATHS: &[&str] = &[
    "/config",
    "/config.php",
    "/conf",
    "/database",
    "/database_backup",
    "/backup",
    "/backup.zip",
    "/api",
    "/api/login",
    "/api/v1",
    "/api/v1/login",
    "/api/user",
    "/api/users",
    "/api/admin",
    "/api/auth",
    "/rest",
    "/rest/login",
    "/private",
    "/secure",
    "/.well-known/security.txt",
    "/.well-known/change-password",
    "/.well-known/apple-app-site-association",
    "/server-status",
    "/status",
    "/server-info",
    "/error",
    "/errors",
    "/403",
    "/404",
    "/500",
    "/401",
];

const TAUNTS: &[&str] = &[
    "My server is more secure than your script is clever. Try again, maybe after learning to code.",
    "Congratulations, you've found the 'waste your time' endpoint.",
    "Your automated scanner is bad and you should feel bad.",
    "You probe like a script kiddie with broken fingers.",
    "Try hacking something your own size, champ.",
    "Wow. Such scan. Very bot. Much blocked.",
    "If stupidity were a crime, your IP would be in jail.",
    "You call that an exploit? My grandma could write better malware.",
    "Your requests are like your skills — rejected.",
    "I don’t speak bot. Try English next time.",
    "Keep poking. Maybe you'll find a vulnerability in your own ego.",
    "The only thing you’ve penetrated is the rate limit.",
    "Access denied. You're not even worth logging.",
    "My firewall does more thinking than your entire script.",
    "Bot detected. Intelligence not detected.",
    "404: Your skills not found.",
    "Error: Brain not initialized.",
    "Try again in your next life.",
    "Scanning? You're just embarrassing yourself.",
    "I've seen toddlers write better attack scripts.",
    "The only thing you're exploiting is your own incompetence.",
    "Even my 404 page is smarter than your crawler.",
    "AI called — it wants you to stop.",
];

const TAUNTS_418: &[&str] = &[
    "This isn't WordPress, it's worse — it's Rust.",
    "Looking for WordPress? You must be lost. This is a real server.",
    "Did you really think you'd find a wp-login.php here? How quaint.",
    "Is that a vulnerability scanner or are you just happy to see my 418 response?",
    "Crawling /wp-content? You must be new here.",
    "418: I'm a teapot, and you're a fool.",
    "418: We serve real humans here.",
    "418: Teapot protocol engaged. No coffee for you.",
    "418: Brew yourself some skills first.",
    "418: Hot water, no mercy.",
    "418: Attack rejected. Tea is sacred.",
];

pub async fn handler(req: Request<Body>, next: Next) -> Response {
    let path = req.uri().path();

    if whitelist::WHITELISTED_PATHS.contains(&path) {
        return next.run(req).await;
    }

    if RESP_418_PATHS.iter().any(|&p| path.starts_with(p)) {
        let mut rng = rand::thread_rng();
        if rng.gen_bool(0.1) {
            return response::im_a_teapot();
        } else {
            let taunt = TAUNTS_418.choose(&mut rng).unwrap_or(&"Go away.");
            return response::error(StatusCode::IM_A_TEAPOT, *taunt);
        }
    } else if RESP_403_PATHS.iter().any(|&p| path.starts_with(p)) {
        let mut rng = rand::thread_rng();
        if rng.gen_bool(0.1) {
            return response::forbidden();
        } else {
            let taunt = TAUNTS.choose(&mut rng).unwrap_or(&"Go away.");
            return response::error(StatusCode::FORBIDDEN, *taunt);
        }
    } else if RESP_400_PATHS.contains(&path) {
        response::bad_request()
    } else {
        next.run(req).await
    }
}