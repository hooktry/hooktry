use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use rusqlite::{Connection, params};
use uuid::Uuid;

use crate::domain::{AssertionResult, Contract, Interaction, Recording};

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
            );
            CREATE TABLE IF NOT EXISTS contracts (
                id TEXT PRIMARY KEY,
                payload TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS assertion_results (
                id TEXT PRIMARY KEY,
                contract_id TEXT NOT NULL,
                interaction_id TEXT NOT NULL,
                passed INTEGER NOT NULL,
                payload TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS assertion_results_contract_interaction
                ON assertion_results(contract_id, interaction_id);",
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
                params![
                    recording.id.to_string(),
                    recording.created_at.to_rfc3339(),
                    payload
                ],
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

    pub fn save_contract(&self, contract: &Contract) {
        let payload = serde_json::to_string(contract).expect("serialize contract");
        self.connection
            .lock()
            .expect("interaction store poisoned")
            .execute(
                "INSERT INTO contracts (id, payload) VALUES (?1, ?2)",
                params![contract.id.to_string(), payload],
            )
            .expect("persist contract");
    }

    pub fn contract(&self, id: Uuid) -> Option<Contract> {
        let connection = self.connection.lock().expect("interaction store poisoned");
        connection
            .query_row(
                "SELECT payload FROM contracts WHERE id = ?1",
                [id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .map(|payload| serde_json::from_str(&payload).expect("deserialize contract"))
    }

    pub fn save_assertion(&self, assertion: &AssertionResult) {
        let payload = serde_json::to_string(assertion).expect("serialize assertion result");
        self.connection
            .lock()
            .expect("interaction store poisoned")
            .execute(
                "INSERT INTO assertion_results
                    (id, contract_id, interaction_id, passed, payload)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    assertion.id.to_string(),
                    assertion.contract_id.to_string(),
                    assertion.interaction_id.to_string(),
                    assertion.passed,
                    payload
                ],
            )
            .expect("persist assertion result");
    }

    pub fn assertion(&self, id: Uuid) -> Option<AssertionResult> {
        let connection = self.connection.lock().expect("interaction store poisoned");
        connection
            .query_row(
                "SELECT payload FROM assertion_results WHERE id = ?1",
                [id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .map(|payload| serde_json::from_str(&payload).expect("deserialize assertion result"))
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
