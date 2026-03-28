CREATE TABLE Issues (
    key TEXT PRIMARY KEY,
    hours INTEGER DEFAULT 0,
    minutes INTEGER DEFAULT 0
);

CREATE TABLE IssueUpdates (
    id INTEGER PRIMARY KEY,
    key TEXT,
    hoursChange INTEGER,
    minutesChange INTEGER
);
