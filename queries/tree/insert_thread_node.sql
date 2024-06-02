INSERT INTO
    thread_node (thread_root_id, uri, did, created_at, post_text)
VALUES
    ($1, $2, $3, $4, $5) RETURNING id;
