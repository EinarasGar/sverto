INSERT INTO connector_provider (kind, display_name) VALUES
    ('ibkr', 'Interactive Brokers')
ON CONFLICT (kind) DO NOTHING;

ALTER TABLE transaction DROP CONSTRAINT IF EXISTS transaction_visibility_check;

ALTER TABLE transaction
    ADD CONSTRAINT transaction_visibility_check
    CHECK (visibility IN ('default', 'ghost', 'hidden', 'action'));

CREATE INDEX IF NOT EXISTS idx_assets_ticker_root_lower ON assets (LOWER(SPLIT_PART(ticker, '.', 1)));
