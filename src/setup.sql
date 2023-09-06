CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

DO
$$
    BEGIN
        IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'queue_status_enum') THEN
            CREATE TYPE "queue_status_enum" as enum ('QUEUED', 'PROCESSING', 'COMPLETED', 'FAILED');
        END IF;
    END
$$;

CREATE TABLE IF NOT EXISTS jobq
(
    id         uuid                     NOT NULL DEFAULT uuid_generate_v4() primary key,
    project_id uuid                     NOT NULL,
    post_id    uuid                     NOT NULL,
    filename   text                     NOT NULL,
    hash       text                     NOT NULL,
    mimetype   text                     NOT NULL,
    sort_order int2                     NOT NULL,
    status     queue_status_enum        NOT NULL,
    started_at timestamp with time zone NOT NULL DEFAULT now(),
    duration   double precision,
    error      text
);

CREATE INDEX IF NOT EXISTS status_idx ON jobq (status);
