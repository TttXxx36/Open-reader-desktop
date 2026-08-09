CREATE TABLE IF NOT EXISTS source_rule_metrics (
  source_id TEXT NOT NULL,
  stage TEXT NOT NULL,
  rule_key TEXT NOT NULL,
  attempts INTEGER NOT NULL DEFAULT 0,
  successes INTEGER NOT NULL DEFAULT 0,
  no_matches INTEGER NOT NULL DEFAULT 0,
  failures INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (source_id, stage, rule_key)
);

CREATE INDEX IF NOT EXISTS idx_source_rule_metrics_stage
  ON source_rule_metrics(stage, rule_key, updated_at DESC);
