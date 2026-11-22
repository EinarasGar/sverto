INSERT INTO account_types (id, account_type_name)
VALUES (12, 'Vehicle')
ON CONFLICT (id) DO NOTHING;
SELECT setval('account_types_id_seq', (SELECT MAX(id) FROM account_types));

INSERT INTO asset_types (id, asset_type_name)
VALUES (9, 'Vehicles')
ON CONFLICT (id) DO NOTHING;
SELECT setval('asset_types_id_seq', (SELECT MAX(id) FROM asset_types));

CREATE TABLE IF NOT EXISTS property (
    id UUID DEFAULT uuidv7() NOT NULL,
    account_id UUID NOT NULL,
    asset_id INT NOT NULL,
    kind VARCHAR NOT NULL,
    CONSTRAINT property_pk PRIMARY KEY (id),
    CONSTRAINT unique_property_account UNIQUE (account_id),
    CONSTRAINT unique_property_asset UNIQUE (asset_id),
    CONSTRAINT property_kind_check CHECK (kind IN ('real_estate', 'vehicle')),
    CONSTRAINT property_account_id_fkey FOREIGN KEY (account_id) REFERENCES account(id) ON DELETE CASCADE,
    CONSTRAINT property_asset_id_fkey FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS property_loan (
    account_id UUID NOT NULL,
    property_id UUID NOT NULL,
    CONSTRAINT property_loan_pk PRIMARY KEY (account_id),
    CONSTRAINT property_loan_account_id_fkey FOREIGN KEY (account_id) REFERENCES account(id) ON DELETE CASCADE,
    CONSTRAINT property_loan_property_id_fkey FOREIGN KEY (property_id) REFERENCES property(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_property_loan_property_id ON property_loan(property_id);
