#![allow(clippy::nonminimal_bool)] // !logged_in!()

mod api_types;

use super::*;
use api_types::*;

use once_cell::sync::Lazy;
use tide::Request;

macro_rules! res {
    (Raw($body:expr, $code:expr)) => {{
        let mut resp = tide::Response::new($code);
        resp.set_body($body);
        Ok(resp)
    }};
    (Raw($body:expr)) => {
        res!(Raw($body, 200))
    };
    (Ok) => {
        Ok(tide::Response::new(200))
    };
    (Err) => {
        Ok(tide::Response::new(500))
    };
    (Err($body:expr)) => {{ res!($body, 500) }};
    (Err($body:expr), $code:expr) => {{ res!(ApiError($body.to_string()), $code) }};
    ($body:expr) => {{ res!($body, 200) }};
    ($body:expr, $code:expr) => {
        match tide::Body::from_json(&$body) {
            Ok(body) => res!(Raw(body, $code)),
            Err(err) => {
                error!("api error: {:?}", err);
                let mut resp = tide::Response::new(500);

                let error_message = {
                    #[cfg(debug_assertions)]
                    {
                        format!("{}", err)
                    }
                    #[cfg(not(debug_assertions))]
                    {
                        String::from("Internal Server Error")
                    }
                };

                // If we fail to serialize the error message we don't send one
                if let Ok(body) = tide::Body::from_json(&ApiError(error_message)) {
                    resp.set_body(body);
                }

                Ok(resp)
            }
        }
    };
}

macro_rules! logged_in {
    ($req:ident) => {
        $req.session().get::<bool>(LOGGED_IN).unwrap_or(false)
    };
}

macro_rules! static_file {
    ($filename:literal) => {
        include_str!(concat!("../../static/", $filename))
    };
}

macro_rules! static_route {
    ($router:ident, $route:literal, $mime:literal, $body:ident) => {
        $router.at($route).get(|_| async {
            let mut body = tide::Body::from_string($body.to_string());
            body.set_mime(concat!($mime, ";charset=UTF-8"));

            res!(Raw(body))
        });
    };
}

const INDEX_HTML: &str = static_file!("index.html");
const ENTRY_HTML: &str = static_file!("entry.html");
const LOGIN_HTML: &str = static_file!("login.html");
const MAIN_CSS: &str = static_file!("main.css");
const API_JS: &str = static_file!("api.js");
const MAIN_JS: &str = static_file!("main.js");
const LOCALIZATIONS_DE: &str = static_file!("localizations_de.json");

const LOGGED_IN: &str = "logged_in";
static SESSION_STORE: Lazy<tide::sessions::MemoryStore> =
    Lazy::new(tide::sessions::MemoryStore::new);

pub fn init(stop_server: oneshot::Receiver<()>) -> ResultJoinHandle {
    task::spawn(async {
        debug!("starting the web server");

        let mut server = tide::new();

        server.with(tide::sessions::SessionMiddleware::new(
            SESSION_STORE.clone(),
            &config!(WEBSERVER_SESSION_SECRET),
        ));

        task::spawn(async {
            loop {
                if let Err(err) = SESSION_STORE.cleanup().await {
                    error!("Failed to remove stale sessions: {:?}", err);
                }

                tokio::time::delay_for(config!(WEBSERVER_REMOVE_SESSIONS_INTERVAL)).await;
            }
        });

        server.at("/").get(tide::Redirect::new("/static/index.html"));

        let mut static_files = server.at("/static");
        static_route!(static_files, "/index.html", "text/html", INDEX_HTML);
        static_route!(static_files, "/entry.html", "text/html", ENTRY_HTML);
        static_route!(static_files, "/login.html", "text/html", LOGIN_HTML);
        static_route!(static_files, "/api.js", "text/javascript", API_JS);
        static_route!(static_files, "/main.js", "text/javascript", MAIN_JS);
        static_route!(static_files, "/main.css", "text/css", MAIN_CSS);

        let mut api = server.at("/api");
        api.at("/entry/:number").get(api_get_entry_number);
        api.at("/entry").post(api_post_entry);
        api.at("/entry/:number").post(api_post_entry_number);
        api.at("/reset_pin/:number").get(api_reset_pin_number);
        api.at("/entries").get(api_get_entries);
        api.at("/logout").get(api_logout);
        api.at("/login").post(api_login);
        api.at("/logged-in").get(api_logged_in);
        api.at("/localizations/:language").get(api_get_localizations);

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

async fn api_get_entry_number(req: Request<()>) -> tide::Result {
    let number = match req.param::<u32>("number") {
        Ok(number) => number,
        Err(_) => return res!(Err("failed to parse number")),
    };

    let entry = if logged_in!(req) {
        get_entry_by_number(number)
    } else {
        get_public_entry_by_number(number)
    };

    match entry {
        Some(entry) => res!(entry),
        None => res!(Err("Not Found"), 404),
    }
}

async fn api_get_localizations(req: Request<()>) -> tide::Result {
    let language = req.param::<String>("language").unwrap();
    let language = match language.as_str() {
        "de" => LOCALIZATIONS_DE,

        _ => return res!(Err("invalid language")),
    };

    res!(Raw(language))
}

async fn api_post_entry(mut req: Request<()>) -> tide::Result {
    if !logged_in!(req) {
        return res!(Err("Not logged in"));
    }

    let mut entry: Entry = match req.body_json().await {
        Ok(body) => body,
        Err(_) => return res!(Err("Failed to deserialize request")),
    };

    {
        // confirm entry format
        if let Err(err) = entry.serialize(&mut Vec::new()) {
            return res!(Err(format!("Entry has invalid format: {:?}", err)));
        }
    }

    if let Some(target) = DATABASE.get(&entry.number) {
        if !(target.client_type == ClientType::Deleted || target.disabled()) {
            return res!(Err("Refused to overwrite existing entry"));
        }
    }

    let current_timestamp = get_current_itelex_timestamp();
    entry.timestamp = current_timestamp; // update the entry's timestamp
    entry.pin = 0; // do _not_ write user supplied pins

    update_entry(entry);

    res!(Ok)
}

async fn api_post_entry_number(mut req: Request<()>) -> tide::Result {
    // update entry at {number}, optionaly moving it to {body.number} if it differs
    if !logged_in!(req) {
        return res!(Err("Not logged in"));
    }

    let number: u32 = match req.param("number") {
        Ok(number) => number,
        Err(_) => return res!(Err("failed to parse number")),
    };

    let mut entry: Entry = match req.body_json().await {
        Ok(body) => body,
        Err(_) => return res!(Err("Failed to deserialize request")),
    };

    {
        // confirm entry format
        if let Err(err) = entry.serialize(&mut Vec::new()) {
            return res!(Err(format!("Entry has invalid format: {:?}", err)));
        }
    }

    if entry.number != number {
        if let Some(target) = DATABASE.get(&entry.number) {
            if !(target.client_type == ClientType::Deleted || target.disabled()) {
                return res!(Err("Refused to overwrite existing target entry"));
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
        return res!(Err("entry does not exist"));
    }; // update the entry's pin

    update_entry(entry); // overwrites old_entry if number == entry.number

    res!(Ok)
}

async fn api_reset_pin_number(req: Request<()>) -> tide::Result {
    if !logged_in!(req) {
        return res!(Err("Not logged in"));
    }

    let number: u32 = match req.param("number") {
        Ok(number) => number,
        Err(_) => return res!(Err("failed to parse number")),
    };

    if let Some(mut entry) = DATABASE.get_mut(&number) {
        entry.pin = 0;
    } else {
        return res!(Err("entry does not exist"));
    }

    res!(Ok)
}

async fn api_get_entries(req: Request<()>) -> tide::Result {
    let result = if logged_in!(req) { get_sanitized_entries() } else { get_public_entries() };

    res!(result)
}

async fn api_logout(mut req: Request<()>) -> tide::Result {
    let session = req.session_mut();
    session.remove(LOGGED_IN);
    session.destroy(); // TODO: is this correct?

    res!(Ok)
}

async fn api_login(mut req: Request<()>) -> tide::Result {
    if logged_in!(req) {
        // we are already logged in and can't be logged in again.

        return res!(Err("Already logged in"));
    }

    if let Ok(body) = req.body_json().await {
        let body: LoginRequest = body;

        let success = body.password == config!(WEBSERVER_PASSWORD);

        if success {
            req.session_mut().insert(LOGGED_IN, true)?;
            res!(LoggedInResponse(success))
        } else {
            res!(Err("Invalid credentials"))
        }
    } else {
        res!(Err("Failed to deserialize request"))
    }
}

async fn api_logged_in(req: Request<()>) -> tide::Result {
    res!(LoggedInResponse(logged_in!(req)))
}
