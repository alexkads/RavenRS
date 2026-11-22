-- What the tests need on the server, as an administrator would set it up for PostgREST.
-- Run as the database's owner. Roles are created once; the schema is made again each run.
-- On a hosted service (`RAVENRS_POSTGREST_HOSTED`) the test leaves the roles block out, since the
-- service has its own, and in a schema other than `api` it drops the tables instead of the schema.
DO $roles$
BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'anon') THEN CREATE ROLE anon NOLOGIN; END IF;
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'authenticated') THEN CREATE ROLE authenticated NOLOGIN; END IF;
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'authenticator') THEN
    CREATE ROLE authenticator LOGIN NOINHERIT PASSWORD 'authenticator-local';
  END IF;
END
$roles$;
GRANT anon, authenticated TO authenticator;

DROP SCHEMA IF EXISTS api CASCADE;
CREATE SCHEMA api;
GRANT USAGE ON SCHEMA api TO anon, authenticated;
SET search_path TO api;

CREATE TABLE plain_q (id BIGINT PRIMARY KEY, name TEXT, seats BIGINT, rate DOUBLE PRECISION, is_active BOOLEAN, day DATE);
CREATE TABLE plain_u (id BIGINT PRIMARY KEY, name TEXT UNIQUE, seats BIGINT, rate DOUBLE PRECISION, is_active BOOLEAN, day DATE);
CREATE TABLE plain_c (id BIGINT PRIMARY KEY, name TEXT, seats BIGINT, rate DOUBLE PRECISION, is_active BOOLEAN, day DATE);
GRANT SELECT, INSERT, UPDATE, DELETE ON plain_q, plain_u, plain_c TO authenticated;

-- Decimals kept exactly, as `ravenrs-app-postgres` makes them.
CREATE TABLE rates (id BIGINT PRIMARY KEY, two NUMERIC(18, 2), six NUMERIC(18, 6));
GRANT SELECT, INSERT, UPDATE, DELETE ON rates TO authenticated;

-- Each row belongs to the tenant in the token, and only it sees or changes the row. The tenant is
-- the `tenant` claim, or the one Supabase Auth puts in `app_metadata`, or else the user (`sub`), as
-- Neon Auth's tokens carry no claim of the application's own.
CREATE FUNCTION token_tenant() RETURNS text LANGUAGE sql STABLE AS $tenant$
  SELECT coalesce(
    current_setting('request.jwt.claims', true)::json ->> 'tenant',
    current_setting('request.jwt.claims', true)::json -> 'app_metadata' ->> 'tenant',
    current_setting('request.jwt.claims', true)::json ->> 'sub')
$tenant$;
GRANT EXECUTE ON FUNCTION token_tenant() TO authenticated;
CREATE TABLE notes (
  id BIGINT PRIMARY KEY,
  tenant TEXT NOT NULL DEFAULT token_tenant(),
  title TEXT
);
ALTER TABLE notes ENABLE ROW LEVEL SECURITY;
CREATE POLICY own_tenant ON notes
  USING (tenant = token_tenant())
  WITH CHECK (tenant = token_tenant());
GRANT SELECT, INSERT, UPDATE, DELETE ON notes TO authenticated;
-- Supabase grants every new table of `public` to `anon` by default; the notes are for those who
-- signed in only.
REVOKE ALL ON notes FROM anon;
