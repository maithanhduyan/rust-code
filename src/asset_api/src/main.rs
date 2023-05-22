use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqliteConnectOptions, SqliteConnection};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
struct Asset {
    id: u32,
    name: String,
    code: String,
    price: f32,
    website: String,
    description: String,
}

#[derive(serde::Deserialize)]
struct CreateAsset {
    name: String,
    code: String,
    price: f32,
    website: String,
    description: String,
}

async fn create_asset(
    asset: web::Json<CreateAsset>,
    conn: web::Data<SqliteConnection>,
) -> impl Responder {
    let new_asset = Asset {
        id: 0,
        name: asset.name.clone(),
        code: asset.code.clone(),
        price: asset.price,
        website: asset.website.clone(),
        description: asset.description.clone(),
    };

    sqlx::query!(
        "INSERT INTO assets (name, code, price, website, description)
        VALUES (?, ?, ?, ?, ?)",
        new_asset.name,
        new_asset.code,
        new_asset.price,
        new_asset.website,
        new_asset.description
    )
    .execute(&**conn)
    .await
    .map_err(|_| HttpResponse::InternalServerError().finish())?;

    HttpResponse::Ok().finish()
}

async fn get_assets(conn: web::Data<SqliteConnection>) -> impl Responder {
    let assets = sqlx::query_as!(
        Asset,
        "SELECT id, name, code, price, website, description FROM assets"
    )
    .fetch_all(&**conn)
    .await
    .map_err(|_| HttpResponse::InternalServerError().finish())?;

    HttpResponse::Ok().json(assets)
}

#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = "sqlite:database.db";

    let mut conn = SqliteConnection::connect_with(
        SqliteConnectOptions::new().filename("assets.db"),
    )
    .await?;

    // Tạo kết nối cơ sở dữ liệu
    // let pool = sqlx::sqlite::SqlitePoolOptions::new()
    //     .connect_with(SqliteConnectOptions::from_str(database_url)?)
    //     .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS assets (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            code TEXT NOT NULL,
            price REAL NOT NULL,
            website TEXT NOT NULL,
            description TEXT NOT NULL
        )",
    )
    .await?;

    HttpServer::new(move || {
        App::new()
            .data(conn.clone())
            .route("/assets", web::post().to(create_asset))
            .route("/assets", web::get().to(get_assets))
    })
    .bind("127.0.0.1:8000")?
    .run()
    .await?;

    Ok(())
}
