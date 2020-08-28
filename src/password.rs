#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct Password {
    hash: [u8; 24],
    salt: [u8; 16],
    cost: u32,
}

impl std::fmt::Debug for Password {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Password [cost={}]", self.cost)
    }
}

use once_cell::sync::Lazy;
static BCRYPT_COST: Lazy<u32> = Lazy::new(Password::calculate_good_cost);
impl Password {
    fn calculate_hash(&self, password: &str) -> [u8; 24] {
        let mut hash = [0u8; 24];
        crypto::bcrypt::bcrypt(self.cost, &self.salt, password.as_bytes(), &mut hash);
        hash
    }

    pub fn validate(&self, password: &str) -> bool {
        self.hash == self.calculate_hash(password)
    }

    fn generate_salt() -> [u8; 16] {
        use rand::Rng;
        let mut salt = [0u8; 16];
        rand::rngs::OsRng.fill(&mut salt);
        salt
    }

    pub fn from_plain(plain: &str) -> Self {
        let mut password =
            Self { salt: Self::generate_salt(), hash: [0u8; 24], cost: *BCRYPT_COST };

        password.hash = password.calculate_hash(plain);

        debug_assert!(password.validate(plain));

        password
    }

    fn calculate_good_cost() -> u32 {
        let mut cost = 5;

        let salt = Self::generate_salt();
        let password = "microbenchmark";

        let wanted_time = config!(MIN_PASSWORD_HASH_TIME);

        let mut probable_result = None;

        loop {
            let mut hash = [0u8; 24];

            let start = std::time::Instant::now();
            crypto::bcrypt::bcrypt(cost, &salt, password.as_bytes(), &mut hash);
            let end = std::time::Instant::now();

            let needed_time = end - start;

            if needed_time >= wanted_time {
                // If we are above the wanted time we have probably arrived at the correct
                // result
                probable_result = Some((cost, needed_time));
                // Check if the previous cost would still satisfy our time constraints (in case
                // of overestimation).
                cost -= 1;
            } else if let Some(probable_result) = probable_result {
                // If we are no longer above the wanted but know the last time we were, we are
                // done.
                info!(
                    "using bycrypt cost {} to hash password which will take ~{}ms",
                    probable_result.0,
                    probable_result.1.as_millis()
                );
                break probable_result.0;
            } else {
                // If we don't know the probable result yet, estimate it.

                let ratio = wanted_time.as_millis() as f64 / needed_time.as_millis() as f64;
                let delta = ratio.log2().ceil() as u32;

                cost += delta;
            }
        }
    }
}

#[test]
fn assert_good_bcrypt_cost() {
    let password = Password::from_plain("microbenchmark");

    let start = std::time::Instant::now();
    password.calculate_hash("microbenchmark");
    let end = std::time::Instant::now();

    let needed_time = (end - start).as_millis();

    assert!(
        needed_time > 250 && needed_time < 1000,
        "password hashing takes a bad ammount of time. Took: {}ms",
        needed_time
    );
}
