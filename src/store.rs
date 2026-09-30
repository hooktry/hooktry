use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use rusqlite::{Connection, params};

use uuid::Uuid;

use crate::domain::{Interaction, Recording};

#[derive(Clone)]
pub struct InteractionStore {
    connection: Arc<Mutex<Connection>>,
}

impl Default for InteractionStore {
    fn default() -> Self {
        Self::in_memory().expect("create in-memory interaction store")
    }
}

impl InteractionStore {
    pub fn in_memory() -> rusqlite::Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        Self::from_connection(Connection::open(path)?)
    }

    fn from_connection(connection: Connection) -> rusqlite::Result<Self> {
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS interactions (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                started_at TEXT NOT NULL,
                payload TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS interactions_session_started
                ON interactions(session_id, started_at);
            CREATE TABLE IF NOT EXISTS recordings (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                payload TEXT NOT NULL
            );",
        )?;

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    pub fn record(&self, interaction: Interaction) {
        let payload = serde_json::to_string(&interaction).expect("serialize interaction");
        self.connection
            .lock()
            .expect("interaction store poisoned")
            .execute(
                "INSERT INTO interactions (id, session_id, started_at, payload)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    interaction.id.to_string(),
                    interaction.session_id.to_string(),
                    interaction.started_at.to_rfc3339(),
                    payload
                ],
            )
            .expect("persist interaction");
    }

    pub fn find(&self, id: Uuid) -> Option<Interaction> {
        let connection = self.connection.lock().expect("interaction store poisoned");
        connection
            .query_row(
                "SELECT payload FROM interactions WHERE id = ?1",
                [id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .map(|payload| serde_json::from_str(&payload).expect("deserialize interaction"))
    }

    pub fn save_recording(&self, recording: &Recording) {
        let payload = serde_json::to_string(recording).expect("serialize recording");
        self.connection
            .lock()
            .expect("interaction store poisoned")
            .execute(
                "INSERT INTO recordings (id, created_at, payload) VALUES (?1, ?2, ?3)",
                params![\n                    recording.id.to_string(),\n                    recording.created_at.to_rfc3339(),\n                    payload\n                ],
            )
            .expect("persist recording");
    }

    pub fn recording(&self, id: Uuid) -> Option<Recording> {
        let connection = self.connection.lock().expect("interaction store poisoned");
        connection
            .query_row(
                "SELECT payload FROM recordings WHERE id = ?1",
                [id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .map(|payload| serde_json::from_str(&payload).expect("deserialize recording"))
    }

    pub fn all(&self) -> Vec<Interaction> {
        let connection = self.connection.lock().expect("interaction store poisoned");
        let mut statement = connection
            .prepare("SELECT payload FROM interactions ORDER BY started_at, id")
            .expect("prepare interaction query");

        statement
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query interactions")
            .map(|payload| {
                serde_json::from_str(&payload.expect("read interaction payload"))
                    .expect("deserialize interaction")
            })
            .collect()
    }
}
