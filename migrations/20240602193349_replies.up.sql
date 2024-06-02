CREATE TYPE edge_type AS enum('quote', 'reply');

ALTER TABLE
    thread_edge
ADD
    COLUMN edge_type edge_type;

UPDATE
    thread_edge
SET
    edge_type = 'quote';

ALTER TABLE
    thread_edge
ALTER COLUMN
    edge_type
SET
    NOT NULL;

DROP INDEX thread_edge_node_idx;

CREATE UNIQUE INDEX thread_edge_node_idx ON thread_edge (
    thread_root_id,
    source_node_id,
    target_node_id,
    edge_type
);

ALTER TABLE
    thread_edge DROP CONSTRAINT thread_edge_thread_root_id_fkey;

ALTER TABLE
    thread_edge
ADD
    CONSTRAINT thread_edge_thread_root_id_fkey FOREIGN KEY (thread_root_id) REFERENCES thread_root;
