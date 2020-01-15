use super::*;

macro_rules! static_file {
    ($filename:literal) => {
        include_str!(concat!("../../static/", $filename))
    };
}
const INDEX_HTML: &str = static_file!("index.html");
const NEW_HTML: &str = static_file!("new.html");
const ENTRY_HTML: &str = static_file!("entry.html");
const MAIN_CSS: &str = static_file!("main.css");
const API_JS: &str = static_file!("api.js");

use tide::{redirect, Request, Response};

macro_rules! static_route {
    ($router:ident, $route:literal, $mime:literal, $body:ident) => {
        $router.at($route).get(|_| async {
            Response::new(200)
                .body_string($body.to_string())
                .set_mime(concat!($mime, ";charset=UTF-8").parse().unwrap())
        });
    };
}

pub fn init(stop_server: oneshot::Receiver<()>) -> ResultJoinHandle {
    task::spawn(async move {
        debug!("starting the web server");

        let mut app = tide::new();
        app.at("/").get(redirect("/static/index.html"));

        let mut static_files = app.at("/static");
        static_route!(static_files, "/index.html", "text/html", INDEX_HTML);
        static_route!(static_files, "/new.html", "text/html", NEW_HTML);
        static_route!(static_files, "/entry.html", "text/html", ENTRY_HTML);
        static_route!(static_files, "/api.js", "text/javascript", API_JS);
        static_route!(static_files, "/main.css", "text/css", MAIN_CSS);

        let mut api = app.at("/api");
        api.at("/entry/:number").post(|req: Request<()>| async move {
            let number = req.param::<u32>("number");
            if let Ok(number) = number {
                Response::new(200).body_json(&get_public_entry_by_number(number).await).unwrap_or(Response::new(400))
            } else {
                Response::new(400)
            }
        });

        api.at("/entries").get(|_| async move {
            Response::new(200).body_json(&get_public_entries().await).unwrap_or(Response::new(400))
        });

        // TODO: move to config
        let addr = "127.0.0.1:8080";

        let listen = app.listen(addr);
        select! {
            res = listen.fuse() => res?,
            _ = stop_server.fuse() => {},
        }

        debug!("stopped the web server");

        Ok(())
    })
}
