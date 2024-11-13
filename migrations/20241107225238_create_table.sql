CREATE TABLE IF NOT EXISTS signatures (
    r BYTEA NOT NULL,
    s BYTEA NOT NULL,
    v INTEGER NOT NULL,
    chain TEXT NOT NULL,
    tx_hash TEXT NOT NULL,
    address TEXT NOT NULL,
    PRIMARY KEY (r, s, v)
);
