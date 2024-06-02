SELECT
    uri,
    id
FROM
    thread_node
WHERE
    thread_root_id = $1;
