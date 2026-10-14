-- 0004 AI: user-configured models (BYOK), conversations and messages
-- (Stage 3 · Model Gateway / Context Engine).

CREATE TABLE model_profiles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    base_url TEXT NOT NULL,
    model TEXT NOT NULL,
    temperature REAL NOT NULL DEFAULT 0.3,
    top_p REAL,
    max_output_tokens INTEGER NOT NULL DEFAULT 2048,
    context_window INTEGER NOT NULL DEFAULT 32000,
    timeout_secs INTEGER NOT NULL DEFAULT 120,
    -- send response_format=json_object for structured tasks (quiz generation etc.)
    json_mode INTEGER NOT NULL DEFAULT 0,
    -- keyring (Windows Credential Manager) | local (fallback, plain text) | none
    key_storage TEXT NOT NULL DEFAULT 'none',
    api_key_local TEXT,
    key_hint TEXT,
    is_default INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

-- The phase-one placeholders were never written; conversations replace them.
DROP TABLE IF EXISTS chat_messages;
DROP TABLE IF EXISTS study_sessions;

CREATE TABLE conversations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    course_id INTEGER NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    -- qa | learn | review | sprint
    mode TEXT NOT NULL DEFAULT 'qa',
    title TEXT NOT NULL DEFAULT '',
    -- auto | full | rag
    context_mode TEXT NOT NULL DEFAULT 'auto',
    -- {"document_ids": [...], "section_ids": [...]}; empty = whole course
    scope TEXT NOT NULL DEFAULT '{}',
    profile_id INTEGER REFERENCES model_profiles(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);
CREATE INDEX idx_conversations_course ON conversations(course_id, updated_at);

CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    -- user | assistant
    role TEXT NOT NULL,
    content TEXT NOT NULL DEFAULT '',
    -- done | streaming | cancelled | error
    status TEXT NOT NULL DEFAULT 'done',
    error TEXT,
    -- JSON array of cited / retrieved sources
    sources TEXT NOT NULL DEFAULT '[]',
    -- JSON: model, context mode used, token estimates, action
    meta TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);
CREATE INDEX idx_messages_conversation ON messages(conversation_id, id);

-- Token accounting for the budget display (estimated when the API reports nothing).
CREATE TABLE ai_usage (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id INTEGER REFERENCES model_profiles(id) ON DELETE SET NULL,
    kind TEXT NOT NULL,
    prompt_tokens INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    estimated INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);
