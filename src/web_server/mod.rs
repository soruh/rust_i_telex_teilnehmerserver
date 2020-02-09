mod api_types;
mod cookie_parser;
mod session;

use super::*;
use api_types::*;
use cookie_parser::Cookies;
use once_cell::sync::Lazy;
use session::{set_session_cookie, SessionKey, SessionKeyLocal};
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
    ($error:expr) => {
        error!($error, 505)
    };

    ($error:expr, $code:expr) => {
        Response::new($code)
            .body_json(&InternalError(String::from($error)))
            .unwrap_or_else(|_| Response::new(505))
    };
}

macro_rules! logged_in {
    ($req:ident) => {
        $req.local::<SessionKeyLocal>().is_some()
    };
}

const INDEX_HTML: &str = static_file!("index.html");
const ENTRY_HTML: &str = static_file!("entry.html");
const LOGIN_HTML: &str = static_file!("login.html");
const MAIN_CSS: &str = static_file!("main.css");
const API_JS: &str = static_file!("api.js");
const MAIN_JS: &str = static_file!("main.js");
const LOCALIZATIONS_DE: &str = static_file!("localizations_de.json");

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

        task::spawn(async move {
            loop {
                SESSION_STORE.remove_old_sessions();

                tokio::time::delay_for(config!(WEBSERVER_REMOVE_SESSIONS_INTERVAL)).await;
            }
        });

        server.at("/").get(redirect("/static/index.html"));

        let mut static_files = server.at("/static");
        static_route!(static_files, "/index.html", "text/html", INDEX_HTML);
        static_route!(static_files, "/entry.html", "text/html", ENTRY_HTML);
        static_route!(static_files, "/login.html", "text/html", LOGIN_HTML);
        static_route!(static_files, "/api.js", "text/javascript", API_JS);
        static_route!(static_files, "/main.js", "text/javascript", MAIN_JS);
        static_route!(static_files, "/main.css", "text/css", MAIN_CSS);

        let mut api = server.at("/api");
        api.at("/entry/:number").get(|req: Request<()>| async move {
            let number = match req.param::<u32>("number") {
                Ok(number) => number,
                Err(_) => return error!("failed to parse number"),
            };

            let entry = if logged_in!(req) {
                get_entry_by_number(number)
            } else {
                get_public_entry_by_number(number)
            };

            match entry {
                Some(entry) => {
                    ok!().body_json(&entry).unwrap_or_else(|_| error!("Failed to serialize result"))
                }
                None => error!("Not Found", 404),
            }
        });

        api.at("/entry").post(|mut req: Request<()>| async move {
            if !logged_in!(req) {
                return error!("Not logged in");
            }

            let mut entry: Entry = match req.body_json().await {
                Ok(body) => body,
                Err(_) => return error!("Failed to deserialize request"),
            };

            {
                use itelex::Serialize;
                // confirm entry format
                if let Err(err) = entry.clone().serialize_le(&mut Vec::new()) {
                    return error!(format!("Entry has invalid format: {:?}", err));
                }
            }

            if let Some(target) = DATABASE.get(&entry.number) {
                if !(target.client_type == ClientType::Deleted || target.disabled()) {
                    return error!(format!("Refused to overwrite existing entry"));
                }
            }

            let current_timestamp = get_current_itelex_timestamp();
            entry.timestamp = current_timestamp; // update the entry's timestamp
            entry.pin = 0; // do _not_ write user supplied pins

            update_entry(entry);

            ok!()
        });

        api.at("/entry/:number").post(|mut req: Request<()>| async move {
            // update entry at {number}, optionaly moving it to {body.number} if it differs
            if !logged_in!(req) {
                return error!("Not logged in");
            }

            let number: u32 = match req.param("number") {
                Ok(number) => number,
                Err(_) => return error!("failed to parse number"),
            };

            let mut entry: Entry = match req.body_json().await {
                Ok(body) => body,
                Err(_) => return error!("Failed to deserialize request"),
            };

            {
                use itelex::Serialize;
                // confirm entry format
                if let Err(err) = entry.clone().serialize_le(&mut Vec::new()) {
                    return error!(format!("Entry has invalid format: {:?}", err));
                }
            }

            if entry.number != number {
                if let Some(target) = DATABASE.get(&entry.number) {
                    if !(target.client_type == ClientType::Deleted || target.disabled()) {
                        return error!(format!("Refused to overwrite existing target entry"));
                    }
                }
            }

            let current_timestamp = get_current_itelex_timestamp();
            entry.timestamp = current_timestamp; // update the entry's timestamp
            entry.pin = if let Some(mut old_entry) = DATABASE.get_mut(&number) {
                let mut old_entry: &mut UnboxedEntry = old_entry.value_mut();
                if entry.number != number {
                    old_entry.client_type = ClientType::Deleted; // delete the old entry
                    old_entry.timestamp = current_timestamp; // set it's timestamp to `now`
                    CHANGED.insert(number, ()); // mark it as changed
                }

                old_entry.pin
            } else {
                return error!("entry does not exist");
            }; // update the entry's pin

            update_entry(entry); // overwrites old_entry if number == entry.number

            ok!()
        });

        api.at("/reset_pin/:number").get(|req: Request<()>| async move {
            if !logged_in!(req) {
                return error!("Not logged in");
            }

            let number: u32 = match req.param("number") {
                Ok(number) => number,
                Err(_) => return error!("failed to parse number"),
            };

            if let Some(mut entry) = DATABASE.get_mut(&number) {
                entry.pin = 0;
            } else {
                return error!("entry does not exist");
            }

            ok!()
        });

        api.at("/entries").get(|req: Request<()>| async move {
            let result =
                if logged_in!(req) { get_sanitized_entries() } else { get_public_entries() };

            ok!().body_json(&result).unwrap_or(error!("failed to serialize result"))
        });

        api.at("/logout").get(|req: Request<()>| async move {
            if let Some(session_key) = req.local::<SessionKeyLocal>() {
                let session_key: SessionKey = session_key.0;

                assert!(SESSION_STORE.drop_session(&session_key).is_some());

                set_session_cookie(
                    ok!(),
                    session_key,
                    config!(WEBSERVER_SESSION_LIFETIME).as_secs() as i64,
                )
            } else {
                // we are already logged out and can't to be logged out again.

                return error!("Not logged in");
            }
        });

        api.at("/login").post(|mut req: Request<()>| async move {
            if logged_in!(req) {
                // we are already logged in and can't be logged in again.

                return error!("Already logged in");
            }

            if let Ok(body) = req.body_json().await {
                let body: LoginRequest = body;

                let success = body.password == config!(WEBSERVER_PASSWORD);

                let res = if success {
                    set_session_cookie(
                        ok!(),
                        SESSION_STORE.new_session(),
                        config!(WEBSERVER_SESSION_LIFETIME).as_secs() as i64,
                    )
                } else {
                    ok!()
                };

                res.body_json(&LoggedInResponse(success))
                    .unwrap_or(error!("Failed to serialize response"))
            } else {
                error!("Failed to deserialize request")
            }
        });

        api.at("/logged-in").get(|req: Request<()>| async move {
            let logged_in = logged_in!(req);

            ok!()
                .body_json(&LoggedInResponse(logged_in))
                .unwrap_or(error!("Failed to serialize response"))
        });

        api.at("/localizations/:language").get(|req: Request<()>| async move {
            let language = req.param::<String>("language").unwrap();
            let language = match language.as_str() {
                "de" => LOCALIZATIONS_DE,

                _ => return error!("invalid language"),
            };

            ok!().body_string(String::from(language))
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
