use super::*;
use std::time::Instant;

pub struct HasSession(pub bool);

type SessionKey = u64;
pub struct SessionMiddleware {
    pub sessions: DashMap<SessionKey, Instant>,
}

impl SessionMiddleware {
    pub fn new() -> Self {
        Self { sessions: DashMap::new() }
    }

    fn generate_key() -> SessionKey {
        rand::random()
    }

    pub fn new_session(&self) -> SessionKey {
        let mut session_key = Self::generate_key();

        while self.sessions.contains_key(&session_key) {
            session_key = Self::generate_key();
        }

        self.sessions.insert(session_key, Instant::now());

        session_key
    }
}

pub fn set_session_cookie(response: Response, key: SessionKey, max_age: i64) -> Response {
    response.set_header("Set-Cookie", format!("session={}; Max-Age={}; Path=/api", key, max_age))
}

impl<State: Send + Sync + 'static> Middleware<State> for &'static SessionMiddleware {
    fn handle<'a>(
        &'a self,
        req: Request<State>,
        next: Next<'a, State>,
    ) -> Pin<Box<dyn Future<Output = Response> + Send + 'a>> {
        Box::pin(async move {
            let session_cookie = req
                .local::<Cookies>()
                .map(|cookies| cookies.iter().find(|cookie| cookie.name() == "session"))
                .flatten();

            dbg!(session_cookie);

            if let Some(session_cookie) = session_cookie {
                let session = session_cookie
                    .value()
                    .parse()
                    .ok()
                    .map(|session_key: SessionKey| self.sessions.get(&session_key))
                    .flatten();

                if let Some(session) = session {
                    set_session_cookie(
                        next.run(req.set_local(HasSession(true))).await,
                        *session.key(),
                        config!(WEBSERVER_SESSION_LIFETIME).as_secs() as i64,
                    )
                } else {
                    // session does not exist

                    // delete session cookie
                    let res = next.run(req.set_local(HasSession(false))).await;
                    set_session_cookie(res, 0, -1)
                }
            } else {
                // user has no session
                next.run(req.set_local(HasSession(false))).await
            }
        })
    }
}
