//! Workers entrypoint for the hike club admin app.
//!
//! Thin by design: this file is Cloudflare-runtime glue (Request/Env/Context,
//! the R2 binding) that only executes inside a deployed/dev worker, so it's
//! excluded from the coverage gate. All business logic lives in `admin`,
//! `validate`, `models` and `store`, unit-tested there without the runtime.
//!
//! Authentication is Cloudflare Access, attached to this Worker in the
//! dashboard — unauthenticated requests never reach this code, so there is
//! deliberately no auth handling here.
pub mod models;
pub mod validate;

use worker::*;

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    Router::new()
        .get_async("/health", |_, _| async { Response::ok("ok") })
        .run(req, env)
        .await
}
