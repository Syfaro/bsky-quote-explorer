INSERT INTO
    thread_root (uri)
VALUES
    ($1) ON CONFLICT (uri) DO
UPDATE
SET
    updated_at = current_timestamp RETURNING id;
