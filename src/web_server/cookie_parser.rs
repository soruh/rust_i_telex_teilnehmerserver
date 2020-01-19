use super::*;

pub type Cookies = Vec<cookie::Cookie<'static>>;

pub struct CookieParser;

//TODO: remove ` + std::fmt::Debug`
impl<State: Send + Sync + std::fmt::Debug + 'static> Middleware<State> for CookieParser {
    fn handle<'a>(
        &'a self,
        req: Request<State>,
        next: Next<'a, State>,
    ) -> Pin<Box<dyn Future<Output = Response> + Send + 'a>> {
        Box::pin(async move {
            let cookies: Cookies = if let Some(cookie) = req.header("Cookie") {
                cookie
                    .split("; ") // TODO: should we split on ';' and `trim` the cookie?
                    .map(|cookie_string| cookie_string.parse())
                    .filter(|cookie| cookie.is_ok())
                    .map(|cookie| cookie.unwrap())
                    .collect()
            } else {
                Vec::new()
            };

            next.run(req.set_local(cookies)).await
        })
    }
}
