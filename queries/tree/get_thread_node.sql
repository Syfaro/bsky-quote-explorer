SELECT
    id
FROM
    thread_node
WHERE
    thread_root_id = $1
    AND uri = $2;
