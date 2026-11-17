# demo-texts.sh: what the demo programs show, for scripts/demo-shots.sh and demo-gifs.sh, which
# source it with $d set and txt NAME writing stdin to $d/NAME.txt. Made up, nothing of this machine.
txt claude <<'EOF'
> Fix the flaky login test in auth/session_test.rs

  I'll read the test first, then run it a few times to see
  how it fails.

  Read auth/session_test.rs (212 lines)
  The test sleeps 50 ms and then checks the session expired.
  On a busy machine the check runs before the timer fires.

  Ran cargo test -p auth 20 times
  3 of 20 failed: expired_session_is_rejected

  The session store already takes a Clock, so the test can
  move time forward instead of waiting for it.

  Update auth/session_test.rs
   - std::thread::sleep(Duration::from_millis(50));
   + clock.advance(Duration::from_secs(61));

  Update auth/session.rs
   - if Instant::now() > self.expires_at {
   + if self.clock.now() > self.expires_at {

  Running the tests beside me:
    leon split --label tests -- cargo test -p auth

  18 passed, 0 failed. Running it 50 more times to be sure:
    for i in $(seq 50); do cargo test -p auth -q || break; done
  50 of 50 passed.

  The same sleep is in two more tests:
    tests/session_test.rs:88   idle_session_expires
    tests/session_test.rs:141  refresh_after_expiry_fails
  Moving both to the test clock.

  Update tests/session_test.rs
   - thread::sleep(Duration::from_secs(2));
   + clock.advance(Duration::from_secs(3601));
EOF
txt tests <<'EOF'
   Compiling auth v0.4.0 (/src/auth)
    Finished `test` profile in 4.21s
     Running unittests src/lib.rs

running 18 tests
test session::tests::refresh_keeps_the_id ... ok
test session::tests::expired_session_is_rejected ... ok
test session::tests::idle_session_expires ... ok
test session::tests::login_sets_a_cookie ... ok
test session::tests::logout_clears_the_cookie ... ok
test token::tests::round_trip ... ok
test token::tests::rejects_a_bad_signature ... ok
test token::tests::rotates_weekly ... ok

test result: ok. 18 passed; 0 failed; finished in 0.31s
EOF
txt server <<'EOF'
listening on http://127.0.0.1:8080
GET /login 200 3ms
POST /session 201 12ms
GET /me 200 2ms
GET /orders 200 9ms
POST /orders 201 21ms
GET /orders/1042 200 4ms
POST /session 401 2ms
POST /session 201 11ms
GET /me 200 2ms
DELETE /session 204 3ms
EOF
txt readme <<'EOF'
> Review the README for the 0.4 release

  Checked every command in README.md against --help: all current.
  Fixed two broken links in the install section.
EOF
txt migrate <<'EOF'
> Migrate the users table to the new schema

  Wrote migrations/0042_users_email_unique.sql
  Dry run on staging: 18,204 rows, 0 conflicts.

  Run it on production now?
EOF
txt api <<'EOF'
> Add rate limiting to the public API

  Read src/routes.rs (88 lines), src/auth.rs (140 lines)
  Every route but /health takes an API key, so the key is the
  natural thing to count by.

  Plan: a token bucket per API key in src/limits.rs, 100 requests
  a minute, with Retry-After on a 429.

  Write src/limits.rs (64 lines)
   + pub struct Bucket { tokens: f64, last: Instant }
   + pub fn per_key(per_minute: u32) -> Limits { ... }

  Write tests/test_limits.py (8 tests)
   + test_101st_request_gets_429
   + test_retry_after_counts_down
   + test_keys_have_separate_buckets

  Ran pytest -q: 42 passed

  Edit src/routes.rs to add the middleware?
     .layer(limits::per_key(100))
EOF
txt deploy <<'EOF'
$ ./deploy.sh api v2.3.1
rolling out api v2.3.1 to 4 pods
pod api-7f9c-1 ready
pod api-7f9c-2 CrashLoopBackOff
rollout failed: 2 of 4 pods crashed
EOF
txt notes <<'EOF'
## 0.4.0

- Sessions survive a server restart
- The login test no longer sleeps
- Rate limits on the public API
EOF
txt build <<'EOF'
$ make release
[ 40%] Building CXX object src/CMakeFiles/core.dir/engine.cpp.o
EOF

txt routes <<'EOF'
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};

use crate::{auth, limits, orders, AppState};

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/orders", get(orders::list).post(orders::create))
        .route("/v1/orders/:id", get(order))
        .layer(limits::per_key(100))
        .layer(auth::api_key())
        .with_state(state)
}

async fn health() -> StatusCode {
    StatusCode::OK
}

async fn order(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<Json<orders::Order>, StatusCode> {
    let order = state.orders.get(id).await.ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(order))
}
EOF
txt pytest <<'EOF'
============================= test session starts ==============================
collected 42 items

tests/test_orders.py ..........                                          [ 23%]
tests/test_limits.py ........                                            [ 42%]
tests/test_users.py ........................                             [100%]

============================== 42 passed in 1.84s ==============================
EOF
txt caddy <<'EOF'
INFO  using config from file  {"file": "Caddyfile"}
INFO  serving initial configuration
INFO  http.log.access  {"method": "GET", "uri": "/health", "status": 200}
EOF
txt htop <<'EOF'
  CPU [||||||||       24.1%]
  Mem [||||||||| 6.1G/16.0G]
  Swp [          0K/2.0G]
  Tasks: 212, 3 running
  Load average: 1.42 1.20 0.98

    PID  CPU% MEM%  Command
   4121  31.0  2.4  api-server
   4180  12.5  1.1  postgres
   4202   3.2  0.4  worker --queue orders
   4210   2.9  0.3  worker --queue emails
   3988   1.4  0.8  redis-server
   4301   0.9  0.2  caddy run
   4122   0.6  0.1  cargo test -p auth
   4377   0.3  0.1  leon server
EOF
txt access <<'EOF'
GET  /health          200   1ms
GET  /login           200   3ms
POST /session         201  12ms
GET  /me              200   2ms
GET  /orders          200   9ms
POST /orders          201  21ms
GET  /orders/1042     200   4ms
GET  /orders/1043     404   2ms
POST /session         401   2ms
POST /session         201  11ms
GET  /me              200   2ms
GET  /health          200   1ms
DELETE /session       204   3ms
GET  /login           200   3ms
POST /session         201  10ms
GET  /orders?page=2   200  14ms
EOF
