-- Integrations and Notifications
-- Migration: 00005_integrations

-- Workspace integrations (YouTrack, GitHub, GitLab, Telegram)
CREATE TABLE workspace_integrations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    integration_type VARCHAR(50) NOT NULL,  -- 'youtrack', 'github', 'gitlab', 'telegram'
    config JSONB NOT NULL,  -- Integration-specific config (tokens encrypted at app level)
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    last_sync_at TIMESTAMPTZ,
    sync_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(workspace_id, integration_type)
);

-- Per-user notification settings per workspace
CREATE TABLE user_notification_settings (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,

    -- Telegram
    telegram_chat_id BIGINT,
    telegram_enabled BOOLEAN DEFAULT FALSE,

    -- What to notify about
    notify_task_assigned BOOLEAN DEFAULT TRUE,
    notify_task_updated BOOLEAN DEFAULT TRUE,
    notify_task_commented BOOLEAN DEFAULT TRUE,
    notify_due_date_approaching BOOLEAN DEFAULT TRUE,
    notify_mention BOOLEAN DEFAULT TRUE,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (user_id, workspace_id)
);

-- Activity log for audit trail
CREATE TABLE activity_log (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id),  -- NULL for system actions
    entity_type VARCHAR(50) NOT NULL,   -- 'task', 'document', 'workspace', 'member'
    entity_id UUID NOT NULL,
    action VARCHAR(50) NOT NULL,        -- 'created', 'updated', 'deleted', 'moved', etc.
    changes JSONB,                      -- Diff of what changed
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_workspace_integrations_type ON workspace_integrations(workspace_id, integration_type);
CREATE INDEX idx_user_notification_settings_user ON user_notification_settings(user_id);
CREATE INDEX idx_activity_log_workspace ON activity_log(workspace_id, created_at DESC);
CREATE INDEX idx_activity_log_entity ON activity_log(entity_type, entity_id);
CREATE INDEX idx_activity_log_user ON activity_log(user_id) WHERE user_id IS NOT NULL;
