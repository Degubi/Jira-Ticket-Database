CREATE TABLE Issues (
    key TEXT PRIMARY KEY,
    minutes INTEGER DEFAULT 0
);

CREATE TABLE IssueUpdates (
    id INTEGER PRIMARY KEY,
    key TEXT,
    minutes INTEGER
);
