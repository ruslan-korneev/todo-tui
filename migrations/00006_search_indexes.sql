-- Full-text and Trigram Search Indexes
-- Migration: 00006_search_indexes

-- Enable trigram extension for fuzzy/ripgrep-like search
CREATE EXTENSION IF NOT EXISTS "pg_trgm";

-- Full-text search index for tasks (title + description)
CREATE INDEX idx_tasks_fts ON tasks USING gin(
    to_tsvector('english', coalesce(title, '') || ' ' || coalesce(description, ''))
);

-- Trigram indexes for fast case-insensitive substring matching
CREATE INDEX idx_tasks_title_trgm ON tasks USING gin(title gin_trgm_ops);
CREATE INDEX idx_tasks_desc_trgm ON tasks USING gin(description gin_trgm_ops)
    WHERE description IS NOT NULL;

-- Full-text search index for documents
CREATE INDEX idx_documents_fts ON documents USING gin(
    to_tsvector('english', coalesce(title, '') || ' ' || coalesce(content, ''))
);

-- Trigram indexes for documents
CREATE INDEX idx_documents_title_trgm ON documents USING gin(title gin_trgm_ops);
CREATE INDEX idx_documents_content_trgm ON documents USING gin(content gin_trgm_ops)
    WHERE content IS NOT NULL;

-- Full-text search for comments
CREATE INDEX idx_comments_fts ON task_comments USING gin(
    to_tsvector('english', content)
);

-- Helper function for combined search ranking
CREATE OR REPLACE FUNCTION search_rank(
    query text,
    title text,
    content text
) RETURNS float AS $$
BEGIN
    RETURN ts_rank(
        to_tsvector('english', coalesce(title, '') || ' ' || coalesce(content, '')),
        plainto_tsquery('english', query)
    );
END;
$$ LANGUAGE plpgsql IMMUTABLE;
