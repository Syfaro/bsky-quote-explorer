INSERT INTO
    did_information (did, also_known_as)
VALUES
    ($1, $2) ON CONFLICT (did) DO
UPDATE
SET
    also_known_as = $2,
    updated_at = current_timestamp;
