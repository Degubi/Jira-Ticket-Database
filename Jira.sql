CREATE TABLE Issues (
    key TEXT PRIMARY KEY,
    minutes INTEGER NOT NULL
);

CREATE TABLE IssueUpdates (
    key TEXT NOT NULL,
    minutes INTEGER NOT NULL
);
