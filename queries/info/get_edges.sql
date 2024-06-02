SELECT
    id,
    source_node_id,
    target_node_id,
    edge_type::text "edge_type!"
FROM
    thread_edge
WHERE
    thread_root_id = $1;
