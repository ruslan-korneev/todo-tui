-- Revert: Workspaces and Membership

DROP INDEX IF EXISTS idx_workspace_invites_email;
DROP INDEX IF EXISTS idx_workspace_invites_token;
DROP INDEX IF EXISTS idx_workspace_members_user;
DROP INDEX IF EXISTS idx_workspaces_slug;
DROP INDEX IF EXISTS idx_workspaces_owner;

DROP TABLE IF EXISTS workspace_invites;
DROP TABLE IF EXISTS workspace_members;
DROP TABLE IF EXISTS workspaces;

DROP TYPE IF EXISTS workspace_role;
