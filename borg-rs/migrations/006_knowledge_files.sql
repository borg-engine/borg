CREATE TABLE IF NOT EXISTS knowledge_files (
  id INTEGER PRIMARY KEY,
  file_name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  size_bytes INTEGER NOT NULL DEFAULT 0,
  inline BOOLEAN NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
