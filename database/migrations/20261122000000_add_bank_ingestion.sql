CREATE TABLE import_sources (
    id         UUID PRIMARY KEY DEFAULT uuidv7(),
    session_id UUID NOT NULL REFERENCES import_sessions(id) ON DELETE CASCADE
               DEFERRABLE INITIALLY DEFERRED,
    kind       TEXT NOT NULL CHECK (kind IN ('file', 'bank_account')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (session_id, id)
);

INSERT INTO import_sources (id, session_id, kind, created_at)
SELECT file_id, session_id, 'file', created_at
FROM import_files;

ALTER TABLE import_parsed_lines DROP CONSTRAINT import_parsed_lines_session_id_file_id_fkey;
ALTER TABLE import_account_mappings DROP CONSTRAINT import_account_mappings_session_id_file_id_fkey;
ALTER TABLE import_assertions DROP CONSTRAINT import_assertions_session_id_file_id_fkey;

ALTER TABLE import_files ADD COLUMN source_id UUID;
UPDATE import_files SET source_id = file_id;
ALTER TABLE import_files
    ALTER COLUMN source_id SET NOT NULL,
    DROP CONSTRAINT import_files_pkey,
    DROP CONSTRAINT import_files_session_id_fkey,
    DROP COLUMN coverage_from,
    DROP COLUMN coverage_to,
    ADD PRIMARY KEY (session_id, source_id),
    ADD FOREIGN KEY (session_id, source_id)
        REFERENCES import_sources(session_id, id) ON DELETE CASCADE;

CREATE TABLE import_bank_accounts (
    session_id     UUID NOT NULL,
    source_id      UUID NOT NULL,
    binding_id     UUID NOT NULL UNIQUE REFERENCES connector_binding(id) ON DELETE CASCADE,
    cutoff_page_id UUID REFERENCES connector_raw_page(id),
    PRIMARY KEY (session_id, source_id),
    FOREIGN KEY (session_id, source_id)
        REFERENCES import_sources(session_id, id) ON DELETE CASCADE
);

ALTER TABLE import_parsed_lines RENAME COLUMN file_id TO source_id;
ALTER TABLE import_account_mappings RENAME COLUMN file_id TO source_id;
ALTER TABLE import_assertions RENAME COLUMN file_id TO source_id;

ALTER INDEX idx_import_parsed_lines_session_file_key
    RENAME TO idx_import_parsed_lines_session_source_key;
ALTER INDEX idx_import_assertions_session_file
    RENAME TO idx_import_assertions_session_source;

ALTER TABLE import_parsed_lines
    ADD FOREIGN KEY (session_id, source_id)
        REFERENCES import_sources(session_id, id) ON DELETE CASCADE;
ALTER TABLE import_account_mappings
    ADD FOREIGN KEY (session_id, source_id)
        REFERENCES import_sources(session_id, id) ON DELETE CASCADE;
ALTER TABLE import_assertions
    ADD FOREIGN KEY (session_id, source_id)
        REFERENCES import_sources(session_id, id) ON DELETE CASCADE;

CREATE TABLE held_transactions (
    id              UUID PRIMARY KEY DEFAULT uuidv7(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    binding_id      UUID NOT NULL REFERENCES connector_binding(id) ON DELETE CASCADE,
    external_id     TEXT NOT NULL,
    external_hash   TEXT NOT NULL,
    asset_id        INT NOT NULL REFERENCES assets(id),
    quantity        DECIMAL NOT NULL,
    date_transacted TIMESTAMPTZ NOT NULL,
    description     TEXT NOT NULL,
    candidate_ids   UUID[] NOT NULL,
    CONSTRAINT unique_held_transactions_external_id UNIQUE (binding_id, external_id)
);
CREATE INDEX idx_held_transactions_user_id ON held_transactions(user_id);

ALTER TABLE connector_binding
    ADD COLUMN history_from DATE,
    ADD COLUMN history_from_last_entry BOOLEAN NOT NULL DEFAULT FALSE;
