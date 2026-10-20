-- Where the user put a collection or series in "My order" (none = after the placed ones).
ALTER TABLE collection_looks ADD COLUMN pos INTEGER;
