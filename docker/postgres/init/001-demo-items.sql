CREATE TABLE IF NOT EXISTS demo_items (
  id BIGSERIAL PRIMARY KEY,
  name TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'active',
  note TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE OR REPLACE FUNCTION set_demo_items_updated_at()
RETURNS trigger AS $$
BEGIN
  NEW.updated_at = now();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS demo_items_set_updated_at ON demo_items;
CREATE TRIGGER demo_items_set_updated_at
BEFORE UPDATE ON demo_items
FOR EACH ROW
EXECUTE FUNCTION set_demo_items_updated_at();

INSERT INTO demo_items (name, status, note)
VALUES ('Alpha PG', 'active', 'postgres demo item')
ON CONFLICT DO NOTHING;
