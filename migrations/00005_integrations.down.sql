-- Revert: Integrations and Notifications

DROP INDEX IF EXISTS idx_activity_log_user;
DROP INDEX IF EXISTS idx_activity_log_entity;
DROP INDEX IF EXISTS idx_activity_log_workspace;
DROP INDEX IF EXISTS idx_user_notification_settings_user;
DROP INDEX IF EXISTS idx_workspace_integrations_type;

DROP TABLE IF EXISTS activity_log;
DROP TABLE IF EXISTS user_notification_settings;
DROP TABLE IF EXISTS workspace_integrations;
