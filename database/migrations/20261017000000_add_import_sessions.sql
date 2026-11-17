CREATE TABLE import_sessions (
    id                  UUID PRIMARY KEY DEFAULT uuidv7(),
    user_id             UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    conversation_id     UUID NOT NULL UNIQUE REFERENCES ai_conversations(id) ON DELETE CASCADE,
    lifecycle           TEXT NOT NULL DEFAULT 'active'
                        CHECK (lifecycle IN ('active', 'completed')),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_import_sessions_user_id ON import_sessions(user_id);

CREATE TABLE import_files (
    session_id      UUID NOT NULL REFERENCES import_sessions(id) DEFERRABLE INITIALLY DEFERRED,
    file_id         UUID NOT NULL REFERENCES user_files(id) DEFERRABLE INITIALLY DEFERRED,
    conversation_id UUID NOT NULL UNIQUE REFERENCES ai_conversations(id) ON DELETE CASCADE,
    status          TEXT NOT NULL DEFAULT 'pending'
                    CHECK (status IN ('pending', 'processing', 'parsed', 'failed')),
    error_message   TEXT,
    paused_until    TIMESTAMPTZ,
    coverage_from   DATE,
    coverage_to     DATE,
    date_order      TEXT CHECK (date_order IN ('day_first', 'month_first')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (session_id, file_id),
    UNIQUE (file_id),
    CHECK ((status = 'failed') = (error_message IS NOT NULL))
);

CREATE TABLE import_account_mappings (
    session_id  UUID NOT NULL,
    file_id     UUID NOT NULL,
    account_ref TEXT NOT NULL,
    account_id  UUID REFERENCES account(id) ON DELETE CASCADE,
    PRIMARY KEY (session_id, file_id, account_ref),
    FOREIGN KEY (session_id, file_id)
        REFERENCES import_files(session_id, file_id) ON DELETE CASCADE
);

CREATE TABLE import_asset_mappings (
    session_id UUID NOT NULL REFERENCES import_sessions(id) ON DELETE CASCADE,
    asset_ref  TEXT NOT NULL,
    asset_id   INT REFERENCES assets(id) ON DELETE CASCADE,
    PRIMARY KEY (session_id, asset_ref)
);

CREATE TABLE import_parsed_lines (
    id                        UUID PRIMARY KEY DEFAULT uuidv7(),
    session_id                UUID NOT NULL,
    file_id                   UUID NOT NULL,
    transaction_key           TEXT,
    account_ref               TEXT NOT NULL,
    date_transacted           TIMESTAMPTZ NOT NULL,
    description               TEXT,
    related_asset_ref         TEXT,
    external_id               TEXT,
    FOREIGN KEY (session_id, file_id)
        REFERENCES import_files(session_id, file_id) ON DELETE CASCADE
);
CREATE INDEX idx_import_parsed_lines_session_file_key
    ON import_parsed_lines(session_id, file_id, transaction_key);

CREATE TABLE import_parsed_legs (
    id             UUID PRIMARY KEY DEFAULT uuidv7(),
    parsed_line_id UUID NOT NULL REFERENCES import_parsed_lines(id) ON DELETE CASCADE,
    asset_ref      TEXT NOT NULL,
    quantity       DECIMAL NOT NULL CHECK (quantity <> 0),
    fee_type       TEXT CHECK (fee_type IN ('transaction', 'exchange', 'withholding_tax'))
);
CREATE INDEX idx_import_parsed_legs_parsed_line_id ON import_parsed_legs(parsed_line_id);

CREATE TABLE import_assertions (
    id          UUID PRIMARY KEY DEFAULT uuidv7(),
    session_id  UUID NOT NULL REFERENCES import_sessions(id) ON DELETE CASCADE,
    file_id     UUID,
    account_ref TEXT,
    asset_ref   TEXT,
    account_id  UUID REFERENCES account(id) ON DELETE CASCADE,
    asset_id    INT REFERENCES assets(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL CHECK (kind IN ('opening', 'closing', 'snapshot', 'value_total')),
    as_of_date  DATE NOT NULL,
    quantity    DECIMAL NOT NULL,
    FOREIGN KEY (session_id, file_id)
        REFERENCES import_files(session_id, file_id) ON DELETE CASCADE,
    CHECK (
        (file_id IS NOT NULL AND account_ref IS NOT NULL AND account_id IS NULL AND asset_id IS NULL)
        OR (file_id IS NULL AND account_ref IS NULL AND asset_ref IS NULL
            AND account_id IS NOT NULL AND asset_id IS NOT NULL AND kind = 'closing')
    ),
    UNIQUE (session_id, account_id, asset_id)
);
CREATE INDEX idx_import_assertions_session_file ON import_assertions(session_id, file_id);

CREATE TABLE import_classification_runs (
    id            UUID PRIMARY KEY DEFAULT uuidv7(),
    session_id    UUID NOT NULL REFERENCES import_sessions(id) ON DELETE CASCADE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    classified_at TIMESTAMPTZ,
    error_message TEXT,
    CHECK (classified_at IS NULL OR error_message IS NULL)
);
CREATE INDEX idx_import_classification_runs_session
    ON import_classification_runs(session_id, created_at);

CREATE TABLE import_line_classifications (
    run_id         UUID NOT NULL REFERENCES import_classification_runs(id) ON DELETE CASCADE,
    parsed_line_id UUID NOT NULL REFERENCES import_parsed_lines(id) ON DELETE CASCADE,
    type_id        INT NOT NULL REFERENCES transaction_types(id),
    category_id    INT REFERENCES transaction_categories(id) DEFERRABLE INITIALLY DEFERRED,
    PRIMARY KEY (run_id, parsed_line_id)
);
CREATE INDEX idx_import_line_classifications_parsed_line
    ON import_line_classifications(parsed_line_id);

CREATE TABLE import_classification_conversations (
    run_id          UUID NOT NULL REFERENCES import_classification_runs(id) ON DELETE CASCADE,
    conversation_id UUID NOT NULL UNIQUE REFERENCES ai_conversations(id) ON DELETE CASCADE,
    PRIMARY KEY (run_id, conversation_id)
);

CREATE TABLE import_staged_decisions (
    id             UUID PRIMARY KEY,
    session_id     UUID NOT NULL REFERENCES import_sessions(id) ON DELETE CASCADE,
    ask_key        TEXT,
    verb           TEXT NOT NULL CHECK (verb IN ('add', 'amend', 'delete', 'skip')),
    transaction_id UUID REFERENCES transaction(id) ON DELETE CASCADE,
    CHECK (verb NOT IN ('amend', 'delete') OR transaction_id IS NOT NULL),
    CHECK (verb <> 'delete' OR ask_key IS NOT NULL),
    UNIQUE (session_id, ask_key, transaction_id)
);
CREATE INDEX idx_import_staged_decisions_transaction_id ON import_staged_decisions(transaction_id);

CREATE TABLE transaction_sources (
    id             UUID PRIMARY KEY DEFAULT uuidv7(),
    user_id        UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    transaction_id UUID NOT NULL REFERENCES transaction(id) ON DELETE CASCADE,
    account_id     UUID NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    asset_id       INT NOT NULL REFERENCES assets(id),
    occurred_on    DATE NOT NULL,
    description    TEXT,
    kind           TEXT NOT NULL CHECK (kind IN ('connector', 'statement', 'receipt', 'manual', 'balance')),
    stream         TEXT NOT NULL,
    external_ref   TEXT,
    itemised_fee   DECIMAL,
    CONSTRAINT unique_transaction_sources_ref UNIQUE (user_id, stream, external_ref, account_id, asset_id)
);
CREATE INDEX idx_transaction_sources_transaction_id ON transaction_sources(transaction_id);
CREATE INDEX idx_transaction_sources_user_occurred ON transaction_sources(user_id, occurred_on);
