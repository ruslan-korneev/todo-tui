-- Revert: Tasks, Statuses, Comments, and Tags

DROP INDEX IF EXISTS idx_task_tags_tag;
DROP INDEX IF EXISTS idx_tags_workspace;
DROP INDEX IF EXISTS idx_task_comments_task;
DROP INDEX IF EXISTS idx_tasks_created_by;
DROP INDEX IF EXISTS idx_tasks_priority;
DROP INDEX IF EXISTS idx_tasks_due_date;
DROP INDEX IF EXISTS idx_tasks_assigned;
DROP INDEX IF EXISTS idx_tasks_status;
DROP INDEX IF EXISTS idx_tasks_workspace;
DROP INDEX IF EXISTS idx_task_statuses_workspace;

DROP TABLE IF EXISTS task_tags;
DROP TABLE IF EXISTS tags;
DROP TABLE IF EXISTS task_comments;
DROP TABLE IF EXISTS tasks;
DROP TABLE IF EXISTS task_statuses;

DROP TYPE IF EXISTS task_priority;
