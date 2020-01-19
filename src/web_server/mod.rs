mod api_types;
mod cookie_parser;
mod session;

use super::*;
use api_types::*;
use cookie_parser::Cookies;
use once_cell::sync::Lazy;
use session::{set_session_cookie, HasSession};
use std::{future::Future, pin::Pin};
use tide::{redirect, Middleware, Next, Request, Response};

macro_rules! static_file {
    ($filename:literal) => {
        include_str!(concat!("../../static/", $filename))
    };
}

macro_rules! ok {
    () => {
        Response::new(200)
    };
}

macro_rules! error {
    ($error:literal) => {
        Response::new(505).body_json(&InternalError($error)).unwrap_or_else(|_| Response::new(505))
    };
}

const INDEX_HTML: &str = static_file!("index.html");
const NEW_HTML: &str = static_file!("new.html");
const ENTRY_HTML: &str = static_file!("entry.html");
const LOGIN_HTML: &str = static_file!("login.html");
const MAIN_CSS: &str = static_file!("main.css");
const API_JS: &str = static_file!("api.js");
const MAIN_JS: &str = static_file!("main.js");

macro_rules! static_route {
    ($router:ident, $route:literal, $mime:literal, $body:ident) => {
        $router.at($route).get(|_| async {
            ok!()
                .body_string($body.to_string())
                .set_mime(concat!($mime, ";charset=UTF-8").parse().unwrap())
        });
    };
}

pub static SESSION_STORE: Lazy<session::SessionMiddleware> =
    Lazy::new(|| session::SessionMiddleware::new());

pub fn init(stop_server: oneshot::Receiver<()>) -> ResultJoinHandle {
    task::spawn(async move {
        debug!("starting the web server");

        let mut server = tide::new();

        server.middleware(cookie_parser::CookieParser);
        server.middleware(Lazy::force(&SESSION_STORE));

        server.at("/").get(redirect("/static/index.html"));

        let mut static_files = server.at("/static");
        static_route!(static_files, "/index.html", "text/html", INDEX_HTML);
        static_route!(static_files, "/new.html", "text/html", NEW_HTML);
        static_route!(static_files, "/entry.html", "text/html", ENTRY_HTML);
        static_route!(static_files, "/login.html", "text/html", LOGIN_HTML);
        static_route!(static_files, "/api.js", "text/javascript", API_JS);
        static_route!(static_files, "/main.js", "text/javascript", MAIN_JS);
        static_route!(static_files, "/main.css", "text/css", MAIN_CSS);

        let mut api = server.at("/api");
        api.at("/entry/:number").get(|req: Request<()>| async move {
            let number = req.param::<u32>("number");
            if let Ok(number) = number {
                ok!()
                    .body_json(&get_public_entry_by_number(number))
                    .unwrap_or_else(|_| error!("Failed to serialize result"))
            } else {
                error!("failed to parse number")
            }
        });

        api.at("/entries").get(|req: Request<()>| async move {
            let logged_in = req.local::<HasSession>().unwrap().0;
            dbg!(logged_in);

            let result = if logged_in { get_entries_without_pin() } else { get_public_entries() };

            ok!().body_json(&result).unwrap_or(error!(""))
        });

        api.at("/login").post(|mut req: Request<()>| async move {
            let logged_in = req.local::<HasSession>().unwrap().0;
            if logged_in {
                // we are already logged in and don't need to be logged in again.

                return ok!();
            }

            if let Ok(body) = req.body_json().await {
                let body: LoginRequest = body;

                dbg!(&body);

                let (res, logged_in) = if body.password == config!(WEBSERVER_PASSWORD) {
                    let session_key = SESSION_STORE.new_session();

                    let res = set_session_cookie(
                        ok!(),
                        session_key,
                        config!(WEBSERVER_SESSION_LIFETIME).as_secs() as i64,
                    );

                    (res, true)
                } else {
                    (ok!(), false)
                };

                res.body_json(&LoggedInResponse(logged_in))
                    .unwrap_or(error!("Failed to serialize response"))
            } else {
                error!("Failed to deserialize request")
            }
        });

        api.at("/logged-in").get(|req: Request<()>| async move {
            let logged_in = req.local::<HasSession>().unwrap().0;

            ok!()
                .body_json(&LoggedInResponse(logged_in))
                .unwrap_or(error!("Failed to serialize response"))
        });

        let addr = SocketAddr::new("0.0.0.0".parse().unwrap(), config!(WEBSERVER_PORT));

        let listen = server.listen(addr);
        select! {
            res = listen.fuse() => res?,
            _ = stop_server.fuse() => {},
        }

        debug!("stopped the web server");

        Ok(())
    })
}
