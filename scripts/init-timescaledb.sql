-- SPECTRE Observability Database Initialization
-- TimescaleDB setup for time-series event storage

-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- Events table (all NATS events)
CREATE TABLE IF NOT EXISTS events (
    time TIMESTAMPTZ NOT NULL,
    event_id UUID NOT NULL,
    event_type VARCHAR(100) NOT NULL,
    source_service VARCHAR(100) NOT NULL,
    target_service VARCHAR(100),
    correlation_id UUID,
    trace_id VARCHAR(64),
    payload JSONB NOT NULL,
    metadata JSONB,
    PRIMARY KEY (time, event_id)
);

-- Convert to hypertable (time-series optimized)
SELECT create_hypertable('events', 'time', if_not_exists => TRUE);

-- Create indexes for common queries
CREATE INDEX IF NOT EXISTS idx_events_event_type ON events (event_type, time DESC);
CREATE INDEX IF NOT EXISTS idx_events_source_service ON events (source_service, time DESC);
CREATE INDEX IF NOT EXISTS idx_events_correlation_id ON events (correlation_id, time DESC) WHERE correlation_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_events_trace_id ON events (trace_id, time DESC) WHERE trace_id IS NOT NULL;

-- GIN index for JSONB payload queries
CREATE INDEX IF NOT EXISTS idx_events_payload ON events USING GIN (payload);

-- Cost tracking table (FinOps)
CREATE TABLE IF NOT EXISTS costs (
    time TIMESTAMPTZ NOT NULL,
    cost_id UUID NOT NULL,
    service VARCHAR(100) NOT NULL,
    provider VARCHAR(50) NOT NULL,  -- 'vertex', 'openai', 'local', etc.
    model VARCHAR(100),
    prompt_tokens INTEGER,
    completion_tokens INTEGER,
    total_tokens INTEGER,
    cost_usd NUMERIC(10, 6),
    user_id VARCHAR(100),
    project_tag VARCHAR(100),
    correlation_id UUID,
    PRIMARY KEY (time, cost_id)
);

SELECT create_hypertable('costs', 'time', if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS idx_costs_service ON costs (service, time DESC);
CREATE INDEX IF NOT EXISTS idx_costs_provider ON costs (provider, time DESC);
CREATE INDEX IF NOT EXISTS idx_costs_project_tag ON costs (project_tag, time DESC) WHERE project_tag IS NOT NULL;

-- Metrics table (system metrics, latencies, etc.)
CREATE TABLE IF NOT EXISTS metrics (
    time TIMESTAMPTZ NOT NULL,
    metric_name VARCHAR(100) NOT NULL,
    metric_value DOUBLE PRECISION NOT NULL,
    service VARCHAR(100) NOT NULL,
    tags JSONB,
    PRIMARY KEY (time, metric_name, service)
);

SELECT create_hypertable('metrics', 'time', if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS idx_metrics_name ON metrics (metric_name, time DESC);
CREATE INDEX IF NOT EXISTS idx_metrics_service ON metrics (service, metric_name, time DESC);

-- Continuous aggregates for cost summaries (materialized views)
CREATE MATERIALIZED VIEW IF NOT EXISTS costs_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', time) AS bucket,
    service,
    provider,
    SUM(cost_usd) as total_cost,
    SUM(total_tokens) as total_tokens,
    COUNT(*) as request_count
FROM costs
GROUP BY bucket, service, provider
WITH NO DATA;

-- Refresh policy (update materialized view automatically)
SELECT add_continuous_aggregate_policy('costs_hourly',
    start_offset => INTERVAL '3 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour',
    if_not_exists => TRUE
);

-- Data retention policies (optional - keep last 90 days)
-- SELECT add_retention_policy('events', INTERVAL '90 days', if_not_exists => TRUE);
-- SELECT add_retention_policy('costs', INTERVAL '90 days', if_not_exists => TRUE);
-- SELECT add_retention_policy('metrics', INTERVAL '90 days', if_not_exists => TRUE);

-- Grant permissions
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO spectre;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO spectre;

-- Success message
DO $$
BEGIN
    RAISE NOTICE 'SPECTRE Observability Database initialized successfully';
END $$;
