use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use rusqlite::{Connection, params};
use tokio::sync::watch;
use uuid::Uuid;

use crate::domain::{
    AssertionResult, Contract, Interaction, Recording, Scenario, ScenarioOutcome, ScenarioRun,
};

#[derive(Clone)]
pub struct InteractionStore {
    connection: Arc<Mutex<Connection>>,
    interaction_revision: watch::Sender<u64>,
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
            CREATE TABLE IF NOT EXISTS interaction_order (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                interaction_id TEXT UNIQUE NOT NULL
            );
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
                ON assertion_results(contract_id, interaction_id);
            CREATE TABLE IF NOT EXISTS scenarios (
                id TEXT PRIMARY KEY,
                payload TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS scenario_runs (
                id TEXT PRIMARY KEY,
                scenario_id TEXT NOT NULL,
                payload TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS scenario_runs_scenario
                ON scenario_runs(scenario_id);
            CREATE TABLE IF NOT EXISTS scenario_outcomes (
                run_id TEXT PRIMARY KEY,
                scenario_id TEXT NOT NULL,
                passed INTEGER NOT NULL,
                payload TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS scenario_outcomes_scenario
                ON scenario_outcomes(scenario_id);",
        )?;
        connection.execute(
            "INSERT INTO interaction_order (interaction_id)
             SELECT interactions.id
             FROM interactions
             LEFT JOIN interaction_order
               ON interaction_order.interaction_id = interactions.id
             WHERE interaction_order.interaction_id IS NULL
             ORDER BY interactions.rowid",
            [],
        )?;

        let (interaction_revision, _) = watch::channel(0);
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            interaction_revision,
        })
    }

    pub fn record(&self, mut interaction: Interaction) {
        let interaction_id = interaction.id.to_string();
        let mut connection = self.connection.lock().expect("interaction store poisoned");
        let transaction = connection
            .transaction()
            .expect("begin interaction transaction");
        transaction
            .execute(
                "INSERT INTO interaction_order (interaction_id) VALUES (?1)",
                [interaction_id.clone()],
            )
            .expect("persist interaction order");
        let sequence = transaction.last_insert_rowid() as u64;
        interaction.observed_sequence = Some(sequence);
        let payload = serde_json::to_string(&interaction).expect("serialize interaction");
        transaction
            .execute(
                "INSERT INTO interactions (id, session_id, started_at, payload)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    interaction_id,
                    interaction.session_id.to_string(),
                    interaction.started_at.to_rfc3339(),
                    payload
                ],
            )
            .expect("persist interaction");
        transaction
            .commit()
            .expect("commit interaction transaction");
        drop(connection);

        let revision = *self.interaction_revision.borrow();
        self.interaction_revision
            .send_replace(revision.wrapping_add(1));
    }

    pub fn subscribe_interactions(&self) -> watch::Receiver<u64> {
        self.interaction_revision.subscribe()
    }

    pub fn find(&self, id: Uuid) -> Option<Interaction> {
        let connection = self.connection.lock().expect("interaction store poisoned");
        connection
            .query_row(
                "SELECT interactions.payload, interaction_order.sequence
                 FROM interactions
                 JOIN interaction_order ON interaction_order.interaction_id = interactions.id
                 WHERE interactions.id = ?1",
                [id.to_string()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?)),
            )
            .ok()
            .map(|(payload, sequence)| deserialize_interaction(&payload, sequence))
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

    pub fn save_scenario(&self, scenario: &Scenario) {
        let payload = serde_json::to_string(scenario).expect("serialize scenario");
        self.connection
            .lock()
            .expect("interaction store poisoned")
            .execute(
                "INSERT INTO scenarios (id, payload) VALUES (?1, ?2)",
                params![scenario.id.to_string(), payload],
            )
            .expect("persist scenario");
    }

    pub fn scenario(&self, id: Uuid) -> Option<Scenario> {
        let connection = self.connection.lock().expect("interaction store poisoned");
        connection
            .query_row(
                "SELECT payload FROM scenarios WHERE id = ?1",
                [id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .map(|payload| serde_json::from_str(&payload).expect("deserialize scenario"))
    }

    pub fn save_scenario_run(&self, run: &ScenarioRun) {
        let payload = serde_json::to_string(run).expect("serialize scenario run");
        self.connection
            .lock()
            .expect("interaction store poisoned")
            .execute(
                "INSERT INTO scenario_runs (id, scenario_id, payload) VALUES (?1, ?2, ?3)",
                params![run.id.to_string(), run.scenario_id.to_string(), payload],
            )
            .expect("persist scenario run");
    }

    pub fn scenario_run(&self, id: Uuid) -> Option<ScenarioRun> {
        let connection = self.connection.lock().expect("interaction store poisoned");
        connection
            .query_row(
                "SELECT payload FROM scenario_runs WHERE id = ?1",
                [id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .map(|payload| serde_json::from_str(&payload).expect("deserialize scenario run"))
    }

    pub fn save_scenario_outcome(&self, outcome: &ScenarioOutcome) {
        let payload = serde_json::to_string(outcome).expect("serialize scenario outcome");
        self.connection
            .lock()
            .expect("interaction store poisoned")
            .execute(
                "INSERT INTO scenario_outcomes (run_id, scenario_id, passed, payload)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    outcome.run_id.to_string(),
                    outcome.scenario_id.to_string(),
                    outcome.passed,
                    payload
                ],
            )
            .expect("persist scenario outcome");
    }

    pub fn scenario_outcome(&self, run_id: Uuid) -> Option<ScenarioOutcome> {
        let connection = self.connection.lock().expect("interaction store poisoned");
        connection
            .query_row(
                "SELECT payload FROM scenario_outcomes WHERE run_id = ?1",
                [run_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .map(|payload| serde_json::from_str(&payload).expect("deserialize scenario outcome"))
    }

    pub fn all_recorded(&self) -> Vec<Interaction> {
        let connection = self.connection.lock().expect("interaction store poisoned");
        let mut statement = connection
            .prepare(
                "SELECT interactions.payload, interaction_order.sequence
                 FROM interaction_order
                 JOIN interactions ON interactions.id = interaction_order.interaction_id
                 ORDER BY interaction_order.sequence",
            )
            .expect("prepare interaction persistence-order query");

        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
            })
            .expect("query interactions in persistence order")
            .map(|row| {
                let (payload, sequence) = row.expect("read interaction payload and sequence");
                deserialize_interaction(&payload, sequence)
            })
            .collect()
    }

    pub fn all(&self) -> Vec<Interaction> {
        let connection = self.connection.lock().expect("interaction store poisoned");
        let mut statement = connection
            .prepare(
                "SELECT interactions.payload, interaction_order.sequence
                 FROM interactions
                 JOIN interaction_order ON interaction_order.interaction_id = interactions.id
                 ORDER BY interactions.started_at, interactions.id",
            )
            .expect("prepare interaction query");

        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
            })
            .expect("query interactions")
            .map(|row| {
                let (payload, sequence) = row.expect("read interaction payload and sequence");
                deserialize_interaction(&payload, sequence)
            })
            .collect()
    }
}

fn deserialize_interaction(payload: &str, sequence: u64) -> Interaction {
    let mut interaction: Interaction =
        serde_json::from_str(payload).expect("deserialize interaction");
    interaction.observed_sequence = Some(sequence);
    interaction
}
