use crate::AppState;
use crate::db::sqlite;
use crate::models::crop_season::CreateCropSeasonRequest;
use actix_web::{HttpResponse, Responder, get, post, put, web};
use serde_json::json;

#[get("/devices/{device_id}/seasons/active")]
async fn get_active_season(
    path: web::Path<String>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    match sqlite::get_active_crop_season(&app_state.sqlite_pool, &device_id).await {
        Ok(season) => HttpResponse::Ok().json(json!({ "status": "success", "data": season })),
        Err(e) => HttpResponse::InternalServerError()
            .json(json!({ "status": "error", "message": e.to_string() })),
    }
}

#[get("/devices/{device_id}/seasons")]
async fn get_seasons_history(
    path: web::Path<String>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    match sqlite::get_crop_seasons_history(&app_state.sqlite_pool, &device_id).await {
        Ok(seasons) => HttpResponse::Ok().json(json!({ "status": "success", "data": seasons })),
        Err(e) => HttpResponse::InternalServerError()
            .json(json!({ "status": "error", "message": e.to_string() })),
    }
}

#[post("/devices/{device_id}/seasons")]
async fn create_season(
    path: web::Path<String>,
    req: web::Json<CreateCropSeasonRequest>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    match sqlite::create_crop_season(&app_state.sqlite_pool, &device_id, req.into_inner()).await {
        Ok(season) => HttpResponse::Ok().json(json!({ "status": "success", "data": season })),
        Err(e) => HttpResponse::InternalServerError()
            .json(json!({ "status": "error", "message": e.to_string() })),
    }
}

#[put("/devices/{device_id}/seasons/active/end")]
async fn end_season(path: web::Path<String>, app_state: web::Data<AppState>) -> impl Responder {
    let device_id = path.into_inner();
    match sqlite::end_active_crop_season(&app_state.sqlite_pool, &device_id).await {
        Ok(_) => {
            HttpResponse::Ok().json(json!({ "status": "success", "message": "Đã kết thúc mùa vụ" }))
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(json!({ "status": "error", "message": e.to_string() })),
    }
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .service(get_active_season)
            .service(get_seasons_history)
            .service(create_season)
            .service(end_season),
    );
}

