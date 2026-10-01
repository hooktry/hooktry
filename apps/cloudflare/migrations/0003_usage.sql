CREATE TABLE usage_events (
  event_id TEXT PRIMARY KEY,
  received_at INTEGER NOT NULL,
  occurred_at_ms INTEGER NOT NULL,
  event_type TEXT NOT NULL CHECK (event_type IN ('scenario_run_completed')),
  passed INTEGER NOT NULL CHECK (passed IN (0, 1)),
  command_success INTEGER NOT NULL CHECK (command_success IN (0, 1)),
  outcome_passed INTEGER NOT NULL CHECK (outcome_passed IN (0, 1)),
  check_count INTEGER NOT NULL,
  contract_count INTEGER NOT NULL,
  exact_cardinality INTEGER NOT NULL CHECK (exact_cardinality IN (0, 1)),
  ranged_cardinality INTEGER NOT NULL CHECK (ranged_cardinality IN (0, 1)),
  ordering_enabled INTEGER NOT NULL CHECK (ordering_enabled IN (0, 1)),
  observation_horizon INTEGER NOT NULL CHECK (observation_horizon IN (0, 1)),
  settle_window INTEGER NOT NULL CHECK (settle_window IN (0, 1)),
  context_match INTEGER NOT NULL CHECK (context_match IN (0, 1)),
  idempotency_context INTEGER NOT NULL CHECK (idempotency_context IN (0, 1)),
  duplicate_guard INTEGER NOT NULL CHECK (duplicate_guard IN (0, 1))
);

CREATE INDEX usage_events_received_at
  ON usage_events(received_at);

CREATE INDEX usage_events_proof_shape
  ON usage_events(duplicate_guard, ordering_enabled, passed);
