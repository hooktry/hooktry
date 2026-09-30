use ortyo::{
    http::{AppState, app},
    store::InteractionStore,
};

#[tokio::main]
async fn main() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:7777")
        .await
        .expect("bind ORTYO HTTP boundary");
    let state = AppState {
        store: InteractionStore::open("ortyo.db").expect("open ORTYO evidence database"),
        ..AppState::default()
    };

    println!("ORTYO HTTP boundary: http://127.0.0.1:7777");
    axum::serve(listener, app(state)).await.expect("serve ORTYO");
}
