use worker::*;

use crate::templates::home_html::render_home;

pub fn handle(_req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    Response::from_html(render_home())
}
