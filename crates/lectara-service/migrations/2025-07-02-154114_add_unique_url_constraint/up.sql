-- Add unique constraint to the url column
CREATE UNIQUE INDEX idx_content_items_url ON content_items(url);
