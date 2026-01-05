-- Drop email verification codes table
DROP TABLE IF EXISTS email_verification_codes;

-- Drop indexes
DROP INDEX IF EXISTS idx_users_username_lower;

-- Remove columns from users table
ALTER TABLE users DROP COLUMN IF EXISTS email_verified_at;
ALTER TABLE users DROP COLUMN IF EXISTS email_verified;
ALTER TABLE users DROP COLUMN IF EXISTS username;
