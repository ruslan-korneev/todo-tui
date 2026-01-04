-- Revert: Full-text and Trigram Search Indexes

DROP FUNCTION IF EXISTS search_rank(text, text, text);

DROP INDEX IF EXISTS idx_comments_fts;
DROP INDEX IF EXISTS idx_documents_content_trgm;
DROP INDEX IF EXISTS idx_documents_title_trgm;
DROP INDEX IF EXISTS idx_documents_fts;
DROP INDEX IF EXISTS idx_tasks_desc_trgm;
DROP INDEX IF EXISTS idx_tasks_title_trgm;
DROP INDEX IF EXISTS idx_tasks_fts;

DROP EXTENSION IF EXISTS "pg_trgm";
