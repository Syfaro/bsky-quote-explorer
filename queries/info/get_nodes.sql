SELECT
    id,
    uri,
    thread_node.did,
    created_at,
    post_text,
    also_known_as
FROM
    thread_node
    LEFT JOIN did_information ON did_information.did = thread_node.did
WHERE
    thread_root_id = $1;
