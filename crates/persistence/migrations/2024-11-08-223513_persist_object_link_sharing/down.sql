-- SQLite does not support `IF EXISTS` on `ALTER TABLE ... DROP COLUMN` (that is PostgreSQL
-- syntax and is a syntax error in SQLite). The columns are always present when this down runs
-- because the up always adds them, so a plain DROP COLUMN is correct.
ALTER TABLE object_permissions DROP COLUMN anyone_with_link_access_level;
ALTER TABLE object_permissions DROP COLUMN anyone_with_link_source;
