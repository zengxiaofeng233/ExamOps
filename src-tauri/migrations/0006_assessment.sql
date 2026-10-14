-- 0006 assessment: quizzes, answers, knowledge mastery, spaced review and the
-- mistake book (Stage 5 · Assessment Engine / Study State Engine).

CREATE TABLE quizzes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    course_id INTEGER NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    -- manual | point | chapter | review | mistakes | step
    origin TEXT NOT NULL DEFAULT 'manual',
    section_id INTEGER REFERENCES document_sections(id) ON DELETE SET NULL,
    step_id INTEGER REFERENCES path_steps(id) ON DELETE SET NULL,
    -- ready (answering) | grading | graded
    status TEXT NOT NULL DEFAULT 'ready',
    allow_skip INTEGER NOT NULL DEFAULT 1,
    -- Percentage 0-100 once graded.
    score REAL,
    -- Subjective answers still waiting for AI or self grading.
    pending INTEGER NOT NULL DEFAULT 0,
    current_index INTEGER NOT NULL DEFAULT 0,
    -- JSON array of SourceRef: the material the questions were written from (S1, S2…)
    sources TEXT NOT NULL DEFAULT '[]',
    -- Last AI grading problem (kept until grading succeeds or the student self-grades)
    grading_error TEXT,
    -- Mastery and the review schedule are updated once, when grading is complete.
    mastery_applied INTEGER NOT NULL DEFAULT 0,
    -- JSON: knowledge point → {before, after, due_date} after grading
    outcome TEXT NOT NULL DEFAULT '[]',
    model TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
    submitted_at TEXT,
    graded_at TEXT
);
CREATE INDEX idx_quizzes_course ON quizzes(course_id, created_at);

CREATE TABLE quiz_questions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    quiz_id INTEGER NOT NULL REFERENCES quizzes(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    -- single_choice | true_false | short_answer | recall | scenario
    kind TEXT NOT NULL,
    knowledge_point_id INTEGER REFERENCES knowledge_points(id) ON DELETE SET NULL,
    prompt TEXT NOT NULL,
    -- JSON array of option texts (single choice)
    options TEXT NOT NULL DEFAULT '[]',
    -- Option letter, "true"/"false", or a reference answer
    answer TEXT NOT NULL DEFAULT '',
    explanation TEXT NOT NULL DEFAULT '',
    -- JSON array of scoring points for subjective questions
    rubric TEXT NOT NULL DEFAULT '[]',
    -- JSON array of SourceRef (where the question comes from)
    sources TEXT NOT NULL DEFAULT '[]',
    response TEXT,
    skipped INTEGER NOT NULL DEFAULT 0,
    -- 0..1 once graded
    score REAL,
    -- auto | ai | self | none
    grading TEXT NOT NULL DEFAULT '',
    feedback TEXT NOT NULL DEFAULT '',
    answered_at TEXT
);
CREATE INDEX idx_quiz_questions_quiz ON quiz_questions(quiz_id, position);
CREATE INDEX idx_quiz_questions_point ON quiz_questions(knowledge_point_id);

-- Mastery estimate and spaced-review schedule per knowledge point.
CREATE TABLE mastery (
    knowledge_point_id INTEGER PRIMARY KEY REFERENCES knowledge_points(id) ON DELETE CASCADE,
    course_id INTEGER NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    -- 0..1, exponential moving average of quiz results
    score REAL NOT NULL DEFAULT 0,
    attempts INTEGER NOT NULL DEFAULT 0,
    correct INTEGER NOT NULL DEFAULT 0,
    -- Consecutive successful reviews (SM-2 repetition count)
    streak INTEGER NOT NULL DEFAULT 0,
    ease REAL NOT NULL DEFAULT 2.5,
    interval_days INTEGER NOT NULL DEFAULT 0,
    due_date TEXT,
    last_result REAL,
    last_quiz_at TEXT,
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);
CREATE INDEX idx_mastery_due ON mastery(course_id, due_date);

-- Mistake book: wrong or weak answers until the student resolves them.
CREATE TABLE mistakes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    course_id INTEGER NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    question_id INTEGER NOT NULL UNIQUE REFERENCES quiz_questions(id) ON DELETE CASCADE,
    knowledge_point_id INTEGER REFERENCES knowledge_points(id) ON DELETE SET NULL,
    resolved INTEGER NOT NULL DEFAULT 0,
    note TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
    resolved_at TEXT
);
CREATE INDEX idx_mistakes_course ON mistakes(course_id, resolved);
