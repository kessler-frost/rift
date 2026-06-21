-- SQLite cannot `ALTER TABLE ... ADD COLUMN ... UNIQUE`, so restore the prior `teams` schema
-- (which had a UNIQUE `server_id` column) via the table-rebuild pattern used elsewhere in these
-- migrations. The dropped `server_id` values are unrecoverable, so the restored column is NULL.
CREATE TABLE IF NOT EXISTS teams_old (
  id integer NOT NULL PRIMARY KEY,
  server_id BIGINTEGER UNIQUE,
  name TEXT NOT NULL,
  server_uid TEXT UNIQUE
);

INSERT INTO teams_old (id, name, server_uid)
SELECT id, name, server_uid
FROM teams;

DROP TABLE teams;

ALTER TABLE teams_old RENAME TO teams;
