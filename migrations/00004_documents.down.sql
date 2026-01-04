-- Revert: Knowledge Base Documents

DROP INDEX IF EXISTS idx_task_document_links_doc;
DROP INDEX IF EXISTS idx_documents_slug;
DROP INDEX IF EXISTS idx_documents_parent;
DROP INDEX IF EXISTS idx_documents_workspace;
DROP INDEX IF EXISTS idx_documents_path;

DROP TABLE IF EXISTS task_document_links;
DROP TABLE IF EXISTS documents;

DROP EXTENSION IF EXISTS "ltree";
