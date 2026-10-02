ALTER TABLE anonymous_exposures
  ADD COLUMN handoff_capability_digest TEXT;

ALTER TABLE anonymous_exposures
  ADD COLUMN handoff_payload_ciphertext TEXT;

ALTER TABLE anonymous_exposures
  ADD COLUMN handoff_payload_nonce TEXT;

ALTER TABLE anonymous_exposures
  ADD COLUMN handoff_expires_at INTEGER;

ALTER TABLE anonymous_exposures
  ADD COLUMN handoff_consumed_at INTEGER;

CREATE UNIQUE INDEX anonymous_exposures_handoff_capability
  ON anonymous_exposures(handoff_capability_digest)
  WHERE handoff_capability_digest IS NOT NULL;
