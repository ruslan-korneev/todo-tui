-- Knowledge Base Documents
-- Migration: 00004_documents

-- Enable ltree extension for hierarchical paths
CREATE EXTENSION IF NOT EXISTS "ltree";

-- Documents table (wiki-style knowledge base)
CREATE TABLE documents (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,

    -- Hierarchical structure using ltree
    -- e.g., 'root.engineering.backend.api_design'
    path ltree NOT NULL,
    parent_id UUID REFERENCES documents(id) ON DELETE CASCADE,

    -- Content
    title VARCHAR(300) NOT NULL,
    slug VARCHAR(100) NOT NULL,
    content TEXT,  -- Markdown

    -- Metadata
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(workspace_id, path)
);

-- Task-Document links (many-to-many)
CREATE TABLE task_document_links (
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (task_id, document_id)
);

-- Indexes for hierarchical queries
CREATE INDEX idx_documents_path ON documents USING gist(path);
CREATE INDEX idx_documents_workspace ON documents(workspace_id);
CREATE INDEX idx_documents_parent ON documents(parent_id);
CREATE INDEX idx_documents_slug ON documents(workspace_id, slug);
CREATE INDEX idx_task_document_links_doc ON task_document_links(document_id);
