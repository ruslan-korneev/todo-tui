-- Revert: Users and Authentication

DROP INDEX IF EXISTS idx_refresh_tokens_hash;
DROP INDEX IF EXISTS idx_refresh_tokens_expires;
DROP INDEX IF EXISTS idx_refresh_tokens_user;
DROP INDEX IF EXISTS idx_users_email;

DROP TABLE IF EXISTS refresh_tokens;
DROP TABLE IF EXISTS users;

DROP EXTENSION IF EXISTS "uuid-ossp";
