ALTER TABLE
    thread_edge DROP CONSTRAINT thread_edge_thread_root_id_fkey;

ALTER TABLE
    thread_edge
ADD
    CONSTRAINT thread_edge_thread_root_id_fkey FOREIGN KEY (thread_root_id) REFERENCES thread_root ON DELETE CASCADE;
