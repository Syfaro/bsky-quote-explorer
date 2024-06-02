INSERT INTO
    thread_edge (thread_root_id, source_node_id, target_node_id)
VALUES
    ($1, $2, $3) ON CONFLICT DO NOTHING;
