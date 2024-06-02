SELECT
    source_node_id,
    target_node_id
FROM
    thread_edge
WHERE
    thread_root_id = $1;
