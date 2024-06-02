INSERT INTO
    thread_edge (thread_root_id, source_node_id, target_node_id, edge_type)
VALUES
    ($1, $2, $3, $4::text::edge_type) ON CONFLICT DO NOTHING;
