-- Remove is_default column from workspaces
-- Migration: 00008_default_workspace

DROP INDEX IF EXISTS idx_workspaces_owner_default;
ALTER TABLE workspaces DROP COLUMN IF EXISTS is_default;
