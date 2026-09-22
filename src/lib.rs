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
mod admin;
pub mod models;
mod r2_store;
mod store;
pub mod validate;

use admin::Outcome;
use r2_store::R2Store;
use worker::*;

/// The admin page. One self-contained file, so it ships in the binary rather
/// than needing a static-assets binding — the same trick hike-club-api uses for
/// its location mapping.
// @spec STORE-012
const INDEX_HTML: &str = include_str!("index.html");

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    Router::new()
        // @spec STORE-013
        .get_async("/health", |_, _| async { Response::ok("ok") })
        .get_async("/", |_, _| async { Response::from_html(INDEX_HTML) })
        .get_async("/api/locations", |_, ctx| async move {
            let store = store(&ctx)?;
            respond(admin::get_locations(&store).await)
        })
        .put_async("/api/locations", |mut req, ctx| async move {
            let store = store(&ctx)?;
            let body = req.bytes().await?;
            respond(admin::put_locations(&store, &body).await)
        })
        .get_async("/api/orphans", |_, ctx| async move {
            let store = store(&ctx)?;
            respond(admin::list_orphans(&store).await)
        })
        .get_async("/api/hikes", |_, ctx| async move {
            let store = store(&ctx)?;
            respond(admin::list_hikes(&store).await)
        })
        .get_async("/api/hikes/:slug", |_, ctx| async move {
            let store = store(&ctx)?;
            respond(admin::get_hike(&store, slug(&ctx)).await)
        })
        .put_async("/api/hikes/:slug", |mut req, ctx| async move {
            let store = store(&ctx)?;
            let body = req.bytes().await?;
            respond(admin::put_hike(&store, slug(&ctx), &body).await)
        })
        .delete_async("/api/hikes/:slug", |_, ctx| async move {
            let store = store(&ctx)?;
            respond(admin::delete_hike(&store, slug(&ctx)).await)
        })
        .get_async("/api/map/:slug", |_, ctx| async move {
            let store = store(&ctx)?;
            respond(admin::get_map(&store, slug(&ctx)).await)
        })
        .put_async("/api/map/:slug", |mut req, ctx| async move {
            let store = store(&ctx)?;
            let content_type = req.headers().get("content-type").ok().flatten();
            let body = req.bytes().await?;
            respond(admin::put_map(&store, slug(&ctx), content_type.as_deref(), body).await)
        })
        .delete_async("/api/map/:slug", |_, ctx| async move {
            let store = store(&ctx)?;
            respond(admin::delete_map(&store, slug(&ctx)).await)
        })
        .run(req, env)
        .await
}

fn store(ctx: &RouteContext<()>) -> Result<R2Store> {
    Ok(R2Store {
        bucket: ctx.env.bucket("HIKES")?,
    })
}

/// The `:slug` param. Absent is impossible for a matched route, and an empty
/// string fails validation in the handler, so this needs no error path.
fn slug(ctx: &RouteContext<()>) -> &str {
    ctx.param("slug").map(String::as_str).unwrap_or_default()
}

// @spec STORE-011
fn respond(outcome: Outcome) -> Result<Response> {
    let mut response = Response::from_bytes(outcome.body)?.with_status(outcome.status);
    response
        .headers_mut()
        .set("content-type", outcome.content_type)?;
    Ok(response)
}
