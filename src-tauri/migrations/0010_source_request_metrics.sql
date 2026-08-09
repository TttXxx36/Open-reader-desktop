CREATE TABLE IF NOT EXISTS source_request_metrics (
  source_id TEXT NOT NULL,
  stage TEXT NOT NULL,
  attempts INTEGER NOT NULL DEFAULT 0,
  successes INTEGER NOT NULL DEFAULT 0,
  failures INTEGER NOT NULL DEFAULT 0,
  cache_hits INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (source_id, stage)
);

CREATE INDEX IF NOT EXISTS idx_source_request_metrics_stage
  ON source_request_metrics(stage, updated_at DESC);
