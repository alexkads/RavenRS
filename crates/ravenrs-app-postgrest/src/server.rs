//! What the database needs for commits: one function that applies a whole unit of work in one
//! transaction. PostgREST runs each call to a function in a transaction of its own, so a failure
//! anywhere leaves nothing behind.
//!
//! It runs as whoever called it (`SECURITY INVOKER`): the grants and row level security that
//! hold for the tables over REST hold here too. Names go through `format('%I')`, and values
//! through `jsonb_populate_record`, which reads each one as its column's type.

/// Installs `ravenrs_commit(changes jsonb)` in the schema PostgREST serves. Run it once, as
/// whoever owns that schema, and then grant `EXECUTE` to the roles that write:
///
/// ```sql
/// SET search_path TO api;  -- the schema PostgREST serves
/// -- SERVER_SQL here
/// REVOKE EXECUTE ON FUNCTION ravenrs_commit(jsonb) FROM PUBLIC;
/// GRANT EXECUTE ON FUNCTION ravenrs_commit(jsonb) TO authenticated;
/// NOTIFY pgrst, 'reload schema';
/// ```
pub const SERVER_SQL: &str = r#"
CREATE OR REPLACE FUNCTION ravenrs_commit(changes jsonb) RETURNS jsonb
LANGUAGE plpgsql SECURITY INVOKER AS $ravenrs$
DECLARE
  change jsonb;
  resource text;
  key_column text;
  key_value jsonb;
  columns text;
  affected bigint;
BEGIN
  FOR change IN SELECT value FROM jsonb_array_elements(changes) LOOP
    resource := change->>'resource';
    key_column := change->>'key_column';
    key_value := jsonb_build_object(key_column, change->'key');
    SELECT string_agg(format('%I', name), ', ') INTO columns
      FROM jsonb_object_keys(CASE WHEN jsonb_typeof(change->'values') = 'object'
                                  THEN change->'values' ELSE '{}'::jsonb END) AS name;
    CASE change->>'op'
    WHEN 'insert' THEN
      EXECUTE format('INSERT INTO %I (%s) SELECT %s FROM jsonb_populate_record(NULL::%I, $1)',
                     resource, columns, columns, resource)
        USING change->'values';
    WHEN 'update' THEN
      IF columns IS NULL THEN
        EXECUTE format('SELECT 1 FROM %I WHERE %I = (SELECT %I FROM jsonb_populate_record(NULL::%I, $1))',
                       resource, key_column, key_column, resource)
          USING key_value;
      ELSE
        EXECUTE format('UPDATE %I SET (%s) = (SELECT %s FROM jsonb_populate_record(NULL::%I, $1)) '
                       'WHERE %I = (SELECT %I FROM jsonb_populate_record(NULL::%I, $2))',
                       resource, columns, columns, resource, key_column, key_column, resource)
          USING change->'values', key_value;
      END IF;
      GET DIAGNOSTICS affected = ROW_COUNT;
      IF affected = 0 THEN
        RAISE EXCEPTION 'a record of % is gone, or is not yours to change', resource
          USING ERRCODE = 'PT409';
      END IF;
    WHEN 'delete' THEN
      EXECUTE format('DELETE FROM %I WHERE %I = (SELECT %I FROM jsonb_populate_record(NULL::%I, $1))',
                     resource, key_column, key_column, resource)
        USING key_value;
    ELSE
      RAISE EXCEPTION 'unknown change %', change->>'op' USING ERRCODE = '22023';
    END CASE;
  END LOOP;
  RETURN jsonb_build_object('applied', jsonb_array_length(changes));
END
$ravenrs$;
"#;
