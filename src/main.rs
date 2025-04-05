use actix_files::NamedFile;
use actix_multipart::Multipart;
use actix_web::{App, HttpRequest, HttpResponse, HttpServer, Responder, Result, get, post, web};
use futures::StreamExt;
use log::info;
use serde::Serialize;
use std::io::Write;

use mime_guess;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use nanoid::nanoid;


const UPLOAD_PASSWORD: &str = "meow";
const PUBLIC_URL: &str = "https://sharex.getdoxxedbyamir.lol";

#[derive(Serialize)]
struct UploadResponse {
    success: bool,
    file_url: String,
}

fn validate_password(req: &actix_web::HttpRequest) -> bool {
    if let Some(auth_header) = req.headers().get("Authorization") {
        if let Ok(auth_value) = auth_header.to_str() {
            return auth_value == format!("Bearer {}", UPLOAD_PASSWORD);
        }
    }
    false
}

#[post("/upload")]
async fn upload(mut payload: Multipart, req: actix_web::HttpRequest) -> impl Responder {
    let id = nanoid!(8);
    if !validate_password(&req) {
        return HttpResponse::Unauthorized().body("Invalid password.");
    }

    let mut file_size = 0;
    let mut saved_filename = String::new();

    while let Some(item) = payload.next().await {
        let mut field = item.unwrap();
        let content_disposition = field.content_disposition().unwrap();

        if content_disposition.get_filename().is_some() {
            //NOTE: i extract file ext here 
            let original_filename = content_disposition.get_filename().unwrap();
            let extension = std::path::Path::new(original_filename)
                .extension()
                .and_then(std::ffi::OsStr::to_str)
                .unwrap_or("");

            saved_filename = if extension.is_empty() {
                id.clone()
            } else {
                format!("{}.{}", id, extension)
            };

            let filepath = format!("./uploads/{}", saved_filename);
            let mut f = std::fs::File::create(filepath).unwrap();

            while let Some(Ok(chunk)) = field.next().await {
                file_size += chunk.len();
                f.write_all(&chunk).unwrap();
            }
        }
    }

    let file_url = format!("{}/file/{}", PUBLIC_URL, saved_filename);

    println!(
        "File uploaded successfully: {}\nFile size: {} bytes",
        file_url, file_size
    );

    HttpResponse::Ok().json(UploadResponse {
        success: true,
        file_url,
    })
}





#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Still alive btw")
}

fn human_readable_size(size: u64) -> String {
    let suffixes = ["B", "KB", "MB", "GB", "TB"];
    let mut size = size as f64;
    let mut index = 0;

    while size >= 1024.0 && index < suffixes.len() - 1 {
        size /= 1024.0;
        index += 1;
    }

    format!("{:.2} {}", size, suffixes[index])
}

#[get("/{filename}")]
async fn get_file_info(filename: web::Path<String>) -> Result<HttpResponse> {
    let filepath = format!("./uploads/{}", filename);
    let path = Path::new(&filepath);

    if !path.exists() {
        return Ok(HttpResponse::NotFound().body("File not found"));
    }

    let metadata = fs::metadata(&path)?;
    let filesize = human_readable_size(metadata.len());
    let modified = metadata
        .modified()
        .map(|time| {
            let datetime: chrono::DateTime<chrono::Utc> = time.into();
            datetime.format("%Y-%m-%d %H:%M:%S").to_string()
        })
        .unwrap_or_else(|_| "Unknown".to_string());

    let mime_type = mime_guess::from_path(&path)
        .first_or_octet_stream()
        .to_string();

    let mut og_tag = String::new();
    let file_url = format!("{}/file/{}", PUBLIC_URL, filename);

    if mime_type.starts_with("image/") {
        og_tag = format!(
            r#"
            <meta property="og:image" content="{url}" />
            <meta name="twitter:card" content="summary_large_image">
            <meta name="twitter:image:src" content="{url}">
            "#,
            url = file_url
        );
    } else if mime_type.starts_with("video/") {
        og_tag = format!(
            r#"
                 <meta property="twitter:player:height" content="626"/>
                 <meta property="twitter:player:width" content="996"/>
                 <meta property="twitter:player:stream" content="{url}"/>
                 <meta property="twitter:player:stream:content_type" content="{mime}"/>
                 <meta property="og:video" content="{url}"/>
                 <meta property="og:video:secure_url" content="{url}"/>
                 <meta property="og:video:height" content="626"/>
                 <meta property="og:video:width" content="996"/>
                 <meta property="og:video:type" content="{mime}"/>
                 <meta property="twitter:image" content="0"/>
                 <meta property="twitter:card" content="player"/>
            "#,
            url = file_url,
            mime = mime_type
        );
    }

    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>{name}</title>
            <meta http-equiv="refresh" content="0; url={url}" />
            <meta property="og:title" content="{name}" />
            <meta property="og:description" content="Size: {size} · Last modified: {modified}" />
            
            <meta property="og:url" content="{url}" />
            {og_tag}
        </head>
        <body>
            <h1>File: {name}</h1>
            <p><strong>Size:</strong> {size} bytes</p>
            <p><strong>Last Modified:</strong> {modified}</p>
            <p><a href="{url}" download>Download File</a></p>
        </body>
        </html>
        "#,
        name = filename,
        size = filesize,
        modified = modified,
        url = file_url,
        og_tag = og_tag
    );

    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

#[get("/file/{filename}")]
async fn get_file(req: HttpRequest, filename: web::Path<String>) -> Result<HttpResponse> {
    let filepath: PathBuf = format!("./uploads/{}", filename.into_inner()).into();
    let mut file = NamedFile::open(filepath)?;

    file = file.set_content_disposition(actix_web::http::header::ContentDisposition {
        disposition: actix_web::http::header::DispositionType::Inline,
        parameters: vec![],
    });

    Ok(file.into_response(&req))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    info!("Starting the server...");

    HttpServer::new(|| {
        App::new()
            .service(hello)
            .service(upload)
            .service(get_file)
            .service(get_file_info)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
