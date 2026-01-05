-- Add is_default column to workspaces
-- Migration: 00008_default_workspace

ALTER TABLE workspaces ADD COLUMN is_default BOOLEAN NOT NULL DEFAULT FALSE;

-- Index for finding user's default workspace quickly
CREATE INDEX idx_workspaces_owner_default ON workspaces(owner_id, is_default) WHERE is_default = TRUE;
