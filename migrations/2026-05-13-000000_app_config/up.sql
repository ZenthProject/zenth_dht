CREATE TABLE app_config (
    key   VARCHAR PRIMARY KEY,
    value VARCHAR NOT NULL
);

INSERT INTO app_config (key, value) VALUES ('required_version', '0.1.0');
