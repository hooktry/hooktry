use ortyo::http::{AppState, app};

#[tokio::main]
async fn main() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:7777")
        .await
        .expect("bind ORTYO HTTP boundary");

    println!("ORTYO HTTP boundary: http://127.0.0.1:7777");
    axum::serve(listener, app(AppState::default()))
        .await
        .expect("serve ORTYO");
}
