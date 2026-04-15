use crate::AppState;
use crate::db::sqlite;
use crate::models::crop_season::CreateCropSeasonRequest;
use actix_web::{HttpResponse, Responder, web};
use serde_json::json;

#[derive(serde::Deserialize)]
pub struct UpdateCropSeasonRequest {
    pub name: String,
    pub plant_type: Option<String>,
    pub description: Option<String>,
}

// handlers giữ nguyên
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

use crate::models::crop_season::CropSeason;

// Hàm cập nhật mùa vụ ĐANG CHẠY (active)
pub async fn update_active_crop_season(
    pool: &sqlx::SqlitePool,
    device_id: &str,
    name: &str,
    plant_type: Option<&str>,
    description: Option<&str>,
) -> Result<CropSeason, sqlx::Error> {
    // 1. Kiểm tra xem có mùa vụ nào đang active không
    let active_season = sqlx::query!(
        "SELECT id FROM crop_seasons WHERE device_id = ? AND status = 'active'",
        device_id
    )
    .fetch_optional(pool)
    .await?;

    match active_season {
        Some(record) => {
            // 2. Cập nhật record đó
            let updated = sqlx::query!(
                r#"
                UPDATE crop_seasons 
                SET name = ?, plant_type = ?, description = ?
                WHERE id = ?
                RETURNING id, device_id, name, plant_type, start_time, end_time, status, description
                "#,
                name,
                plant_type,
                description,
                record.id
            )
            .fetch_one(pool)
            .await?;

            Ok(CropSeason {
                id: updated.id,
                device_id: updated.device_id,
                name: updated.name,
                plant_type: updated.plant_type,
                start_time: updated.start_time,
                end_time: updated.end_time,
                status: updated.status,
                description: updated.description,
            })
        }
        None => Err(sqlx::Error::RowNotFound),
    }
}

async fn update_season(
    path: web::Path<String>,
    req: web::Json<UpdateCropSeasonRequest>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    let data = req.into_inner();

    match sqlite::update_active_crop_season(
        &app_state.sqlite_pool,
        &device_id,
        &data.name,
        data.plant_type.as_deref(),
        data.description.as_deref(),
    )
    .await
    {
        Ok(season) => HttpResponse::Ok().json(json!({ "status": "success", "data": season })),
        Err(sqlx::Error::RowNotFound) => HttpResponse::NotFound().json(
            json!({ "status": "error", "message": "Không tìm thấy mùa vụ đang chạy để sửa" }),
        ),
        Err(e) => HttpResponse::InternalServerError()
            .json(json!({ "status": "error", "message": e.to_string() })),
    }
}

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
    cfg.route("/seasons", web::get().to(get_seasons_history))
        .route("/seasons", web::post().to(create_season))
        .route("/seasons/active", web::get().to(get_active_season))
        .route("/seasons/active", web::put().to(update_season))
        .route("/seasons/active/end", web::put().to(end_season));
}
