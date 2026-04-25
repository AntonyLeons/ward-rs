pub mod config;
pub mod models;
pub mod system;

use axum::http::{HeaderValue, header};
use axum::{
    Json, Router,
    extract::DefaultBodyLimit,
    extract::State,
    response::{Html, IntoResponse},
    routing::{get, post},
};
use rust_embed::RustEmbed;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::set_header::SetResponseHeaderLayer;

#[derive(RustEmbed)]
#[folder = "assets/"]
struct Assets;

use crate::config::ConfigManager;
use crate::models::{InfoDto, ResponseDto, SetupDto, Theme, UptimeDto, UsageDto};
use crate::system::SystemMonitor;

struct AppState {
    sys_monitor: Arc<Mutex<SystemMonitor>>,
    config_manager: Arc<ConfigManager>,
    active_port: String,
    port_overridden: bool,
}

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about = "Ward dashboard rewrite in Rust", long_about = None)]
struct Args {
    /// Port to run the server on (1024-65535)
    #[arg(short, long, value_parser = port_in_range)]
    port: Option<u16>,
}

fn port_in_range(s: &str) -> Result<u16, String> {
    let port: usize = s
        .parse()
        .map_err(|_| format!("`{s}` isn't a valid port number"))?;
    if (1024..=65535).contains(&port) {
        Ok(port as u16)
    } else {
        Err(format!("Port not in range 1024-65535 (provided {port})"))
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let sys_monitor = Arc::new(Mutex::new(SystemMonitor::new()));
    let config_manager = Arc::new(ConfigManager::new("setup.ini"));

    let env_name = std::env::var("WARD_NAME").ok();
    let env_theme = std::env::var("WARD_THEME").ok();
    let env_port = std::env::var("WARD_PORT").ok();
    let env_fog = std::env::var("WARD_FOG").ok();
    let env_bg = std::env::var("WARD_BACKGROUND").ok();

    let has_env_config = env_name.is_some()
        || env_theme.is_some()
        || env_port.is_some()
        || env_fog.is_some()
        || env_bg.is_some();

    if has_env_config {
        let setup_dto = SetupDto {
            server_name: env_name.unwrap_or_else(|| "Ward".to_string()),
            theme: env_theme
                .as_deref()
                .unwrap_or("light")
                .parse()
                .unwrap_or(Theme::Light),
            port: env_port
                .as_deref()
                .unwrap_or("4000")
                .parse::<u16>()
                .unwrap_or(4000),
            enable_fog: env_fog
                .as_deref()
                .unwrap_or("true")
                .parse::<bool>()
                .unwrap_or(true),
            background_color: env_bg.unwrap_or_else(|| "default".to_string()),
        };
        if setup_dto.validate().is_ok() {
            let _ = config_manager.write_config(&setup_dto);
        }
    }

    let port_from_cli = args.port;
    let port_from_env = env_port.and_then(|p| p.parse::<u16>().ok());

    let port_overridden = port_from_cli.is_some() || port_from_env.is_some();

    let port = port_from_cli
        .or(port_from_env)
        .unwrap_or_else(|| config_manager.read_config().map(|c| c.port).unwrap_or(4000));

    let app_state = Arc::new(AppState {
        sys_monitor,
        config_manager: config_manager.clone(),
        active_port: port.to_string(),
        port_overridden,
    });

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/info", get(info_handler))
        .route("/api/usage", get(usage_handler))
        .route("/api/uptime", get(uptime_handler))
        .route("/api/setup", post(setup_handler))
        .route("/css/{*file}", get(static_handler))
        .route("/js/{*file}", get(static_handler))
        .route("/img/{*file}", get(static_handler))
        .route("/fonts/{*file}", get(static_handler))
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_XSS_PROTECTION,
            HeaderValue::from_static("1; mode=block"),
        ))
        .with_state(app_state);

    let addr = format!("0.0.0.0:{port}");
    tracing::info!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

use askama::Template;

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    theme: Theme,
    enable_fog: bool,
    background_color: String,
    server_name: String,
    version: String,
    info: InfoDto,
    uptime: UptimeDto,
}

#[derive(Template)]
#[template(path = "setup.html")]
struct SetupTemplate {
    port: String,
    port_overridden: bool,
}

async fn static_handler(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(axum::http::header::CONTENT_TYPE, mime.as_ref())],
                content.data,
            )
                .into_response()
        }
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
}

async fn index_handler(State(state): State<Arc<AppState>>) -> Html<String> {
    if !state.config_manager.is_configured() {
        let tmpl = SetupTemplate {
            port: state.active_port.clone(),
            port_overridden: state.port_overridden,
        };
        return Html(
            tmpl.render()
                .unwrap_or_else(|_| "Internal Server Error".to_string()),
        );
    }

    let config = state
        .config_manager
        .read_config()
        .unwrap_or_else(|| SetupDto {
            server_name: "Ward".to_string(),
            theme: Theme::Light,
            port: state.active_port.parse::<u16>().unwrap_or(4000),
            enable_fog: true,
            background_color: "default".to_string(),
        });
    let monitor = state.sys_monitor.lock().await;

    let tmpl = IndexTemplate {
        theme: config.theme,
        enable_fog: config.enable_fog,
        background_color: config.background_color,
        server_name: config.server_name,
        version: format!("{} (Rust)", env!("CARGO_PKG_VERSION")),
        info: monitor.get_info(),
        uptime: monitor.get_uptime(),
    };

    Html(
        tmpl.render()
            .unwrap_or_else(|_| "Internal Server Error".to_string()),
    )
}

#[allow(dead_code)]
async fn setup_page_handler(State(state): State<Arc<AppState>>) -> Html<String> {
    let tmpl = SetupTemplate {
        port: state.active_port.clone(),
        port_overridden: state.port_overridden,
    };
    Html(
        tmpl.render()
            .unwrap_or_else(|_| "Internal Server Error".to_string()),
    )
}

async fn info_handler(State(state): State<Arc<AppState>>) -> Json<InfoDto> {
    let monitor = state.sys_monitor.lock().await;
    Json(monitor.get_info())
}

async fn usage_handler(State(state): State<Arc<AppState>>) -> Json<UsageDto> {
    let monitor = state.sys_monitor.lock().await;
    Json(monitor.get_usage())
}

async fn uptime_handler(State(state): State<Arc<AppState>>) -> Json<UptimeDto> {
    let monitor = state.sys_monitor.lock().await;
    Json(monitor.get_uptime())
}

async fn setup_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SetupDto>,
) -> impl IntoResponse {
    if state.config_manager.is_configured() {
        return (
            axum::http::StatusCode::OK,
            Json(ResponseDto {
                message: "Application already configured".to_string(),
            }),
        );
    }

    if let Err(message) = payload.validate() {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(ResponseDto { message }),
        );
    }

    match state.config_manager.write_config(&payload) {
        Ok(_) => (
            axum::http::StatusCode::OK,
            Json(ResponseDto {
                message: "Settings saved correctly".to_string(),
            }),
        ),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(ResponseDto {
                message: format!("Failed to save settings: {e}"),
            }),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        body::to_bytes,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    fn test_app() -> Router {
        let sys_monitor = Arc::new(Mutex::new(SystemMonitor::new()));
        let config_manager = Arc::new(ConfigManager::new("test_integration.ini"));
        let _ = std::fs::remove_file("test_integration.ini"); // ensure clean

        let app_state = Arc::new(AppState {
            sys_monitor,
            config_manager,
            active_port: "4000".to_string(),
            port_overridden: false,
        });

        Router::new()
            .route("/", get(index_handler))
            .route("/api/info", get(info_handler))
            .route("/api/usage", get(usage_handler))
            .route("/api/uptime", get(uptime_handler))
            .route("/api/setup", post(setup_handler))
            .layer(DefaultBodyLimit::max(16 * 1024))
            .with_state(app_state)
    }

    #[tokio::test]
    async fn test_index_unconfigured() {
        let app = test_app();

        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_api_info() {
        let app = test_app();

        let request = Request::builder()
            .uri("/api/info")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_api_usage() {
        let app = test_app();

        let request = Request::builder()
            .uri("/api/usage")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_api_uptime() {
        let app = test_app();

        let request = Request::builder()
            .uri("/api/uptime")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_api_setup() {
        let app = test_app();

        let setup_json = r#"{
            "serverName": "TestServer",
            "theme": "dark",
            "port": 4000,
            "enableFog": true,
            "backgroundColor": "default"
        }"#;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/setup")
                    .header("content-type", "application/json")
                    .body(Body::from(setup_json))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Clean up
        let _ = std::fs::remove_file("test_integration.ini");
    }

    #[tokio::test]
    async fn test_api_setup_validation_error() {
        let app = test_app();

        let setup_json = r#"{
            "serverName": "",
            "theme": "dark",
            "port": 4000,
            "enableFog": true,
            "backgroundColor": "default"
        }"#;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/setup")
                    .header("content-type", "application/json")
                    .body(Body::from(setup_json))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let dto: ResponseDto = serde_json::from_slice(&body).unwrap();
        assert!(dto.message.contains("serverName"));

        let _ = std::fs::remove_file("test_integration.ini");
    }

    #[tokio::test]
    async fn test_api_setup_body_limit() {
        let app = test_app();

        let big_name = "a".repeat(20 * 1024);
        let setup_json = format!(
            r#"{{
            "serverName": "{big_name}",
            "theme": "dark",
            "port": 4000,
            "enableFog": true,
            "backgroundColor": "default"
        }}"#
        );

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/setup")
                    .header("content-type", "application/json")
                    .body(Body::from(setup_json))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);

        let _ = std::fs::remove_file("test_integration.ini");
    }
}
