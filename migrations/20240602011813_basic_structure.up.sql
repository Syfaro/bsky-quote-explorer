CREATE TABLE did_information (
    did TEXT PRIMARY KEY,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT current_timestamp,
    also_known_as TEXT
);

CREATE TABLE thread_root (
    id BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    uri TEXT NOT NULL UNIQUE,
    initialized_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT current_timestamp
);

CREATE TABLE thread_node (
    id BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    thread_root_id BIGINT NOT NULL REFERENCES thread_root ON DELETE CASCADE,
    uri TEXT NOT NULL,
    did TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    post_text TEXT NOT NULL
);

CREATE UNIQUE INDEX thread_node_uri_idx ON thread_node (thread_root_id, uri);

CREATE TABLE thread_edge (
    id BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    thread_root_id BIGINT NOT NULL REFERENCES thread_edge ON DELETE CASCADE,
    source_node_id BIGINT NOT NULL REFERENCES thread_node ON DELETE CASCADE,
    target_node_id BIGINT NOT NULL REFERENCES thread_node ON DELETE CASCADE
);

CREATE UNIQUE INDEX thread_edge_node_idx ON thread_edge (thread_root_id, source_node_id, target_node_id);
