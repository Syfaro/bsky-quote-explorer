ALTER TABLE
    thread_edge DROP COLUMN edge_type;

DROP TYPE edge_type;

CREATE UNIQUE INDEX thread_edge_node_idx ON thread_edge (thread_root_id, source_node_id, target_node_id);
