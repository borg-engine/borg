-- Add attempt tracking to task_outputs so each row is keyed by (task_id, attempt, phase).
ALTER TABLE task_outputs ADD COLUMN attempt INTEGER NOT NULL DEFAULT 0;
