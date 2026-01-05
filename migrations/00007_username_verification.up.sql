-- Add username column to users table
ALTER TABLE users ADD COLUMN username VARCHAR(30) UNIQUE;

-- Create index for case-insensitive username lookups
CREATE UNIQUE INDEX idx_users_username_lower ON users(LOWER(username));

-- Add email verification columns
ALTER TABLE users ADD COLUMN email_verified BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE users ADD COLUMN email_verified_at TIMESTAMPTZ;

-- Create email verification codes table
CREATE TABLE email_verification_codes (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code VARCHAR(6) NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    used_at TIMESTAMPTZ
);

CREATE INDEX idx_verification_codes_user ON email_verification_codes(user_id);
CREATE INDEX idx_verification_codes_expires ON email_verification_codes(expires_at);
