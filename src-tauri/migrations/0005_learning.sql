-- 0005 learning: knowledge points, learning paths, course videos, notes links,
-- teaching conversations and study state (Stage 4 · Tutor / Learning Path / Study State).

ALTER TABLE courses ADD COLUMN exam_date TEXT;

CREATE TABLE knowledge_points (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    course_id INTEGER NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    document_id INTEGER REFERENCES documents(id) ON DELETE SET NULL,
    section_id INTEGER REFERENCES document_sections(id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    summary TEXT NOT NULL DEFAULT '',
    page_start INTEGER,
    page_end INTEGER,
    -- 1 normal · 2 important · 3 key exam point
    importance INTEGER NOT NULL DEFAULT 1,
    -- manual | heading | ai
    origin TEXT NOT NULL DEFAULT 'manual',
    position INTEGER NOT NULL DEFAULT 0,
    -- new (未学) | learned (学过) | mastered (已掌握, set by quiz results)
    status TEXT NOT NULL DEFAULT 'new',
    learned_at TEXT,
    difficult INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);
CREATE INDEX idx_knowledge_points_course ON knowledge_points(course_id, position);
CREATE INDEX idx_knowledge_points_section ON knowledge_points(section_id);

-- External course videos / links; the app never embeds the website.
CREATE TABLE resources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    course_id INTEGER NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    -- video | link
    kind TEXT NOT NULL DEFAULT 'video',
    title TEXT NOT NULL,
    url TEXT NOT NULL DEFAULT '',
    section_id INTEGER REFERENCES document_sections(id) ON DELETE SET NULL,
    -- subtitles / transcript pasted by the user: the only video content the AI may use
    transcript TEXT NOT NULL DEFAULT '',
    notes TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);
CREATE INDEX idx_resources_course ON resources(course_id);

CREATE TABLE resource_marks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    resource_id INTEGER NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
    seconds INTEGER NOT NULL,
    label TEXT NOT NULL DEFAULT '',
    note TEXT NOT NULL DEFAULT '',
    knowledge_point_id INTEGER REFERENCES knowledge_points(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);
CREATE INDEX idx_resource_marks_resource ON resource_marks(resource_id, seconds);

CREATE TABLE learning_paths (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    course_id INTEGER NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    -- learn | review | sprint
    mode TEXT NOT NULL DEFAULT 'learn',
    -- manual | template | ai
    origin TEXT NOT NULL DEFAULT 'manual',
    archived INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);
CREATE INDEX idx_learning_paths_course ON learning_paths(course_id);

CREATE TABLE path_steps (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path_id INTEGER NOT NULL REFERENCES learning_paths(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    -- read_text | read_slides | video | ai_explain | quiz | recall | review | custom
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    instructions TEXT NOT NULL DEFAULT '',
    document_id INTEGER REFERENCES documents(id) ON DELETE SET NULL,
    section_id INTEGER REFERENCES document_sections(id) ON DELETE SET NULL,
    resource_id INTEGER REFERENCES resources(id) ON DELETE SET NULL,
    knowledge_point_ids TEXT NOT NULL DEFAULT '[]',
    estimated_minutes INTEGER,
    -- todo | in_progress | done | skipped
    status TEXT NOT NULL DEFAULT 'todo',
    started_at TEXT,
    completed_at TEXT,
    note TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);
CREATE INDEX idx_path_steps_path ON path_steps(path_id, position);

ALTER TABLE conversations ADD COLUMN section_id INTEGER REFERENCES document_sections(id) ON DELETE SET NULL;
ALTER TABLE conversations ADD COLUMN step_id INTEGER REFERENCES path_steps(id) ON DELETE SET NULL;
ALTER TABLE conversations ADD COLUMN resource_id INTEGER REFERENCES resources(id) ON DELETE SET NULL;
-- 从零学习 progress over lesson parts: {"next", "parts", "last", "taught", "skipped", "finished"}
ALTER TABLE conversations ADD COLUMN progress TEXT NOT NULL DEFAULT '{}';

ALTER TABLE notes ADD COLUMN section_id INTEGER REFERENCES document_sections(id) ON DELETE SET NULL;
ALTER TABLE notes ADD COLUMN knowledge_point_id INTEGER REFERENCES knowledge_points(id) ON DELETE SET NULL;
ALTER TABLE notes ADD COLUMN page_number INTEGER;

-- Where the student is in each course (restored on the next launch).
CREATE TABLE study_state (
    course_id INTEGER PRIMARY KEY REFERENCES courses(id) ON DELETE CASCADE,
    active_path_id INTEGER REFERENCES learning_paths(id) ON DELETE SET NULL,
    current_step_id INTEGER REFERENCES path_steps(id) ON DELETE SET NULL,
    current_section_id INTEGER REFERENCES document_sections(id) ON DELETE SET NULL,
    current_knowledge_point_id INTEGER REFERENCES knowledge_points(id) ON DELETE SET NULL,
    last_document_id INTEGER REFERENCES documents(id) ON DELETE SET NULL,
    last_page INTEGER,
    last_conversation_id INTEGER REFERENCES conversations(id) ON DELETE SET NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

-- Learning history for progress charts and recommendations.
CREATE TABLE study_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    course_id INTEGER NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    -- read | explain | lesson_finished | review_session | sprint_session | step_done | step_skipped
    -- (Stage 5 adds quiz events)
    kind TEXT NOT NULL,
    knowledge_point_id INTEGER,
    step_id INTEGER,
    section_id INTEGER,
    document_id INTEGER,
    detail TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);
CREATE INDEX idx_study_events_course ON study_events(course_id, created_at);
