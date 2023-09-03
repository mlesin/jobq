DO
$$
    BEGIN
        IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'status_enum') THEN
            CREATE TYPE "status_enum" as enum ('QUEUED', 'PROCESSING', 'COMPLETED', 'FAILED');
        END IF;
        IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'priority_enum') THEN
            CREATE TYPE "priority_enum" as enum ('HIGH', 'NORMAL', 'LOW');
        END IF;
    END
$$;

CREATE TABLE IF NOT EXISTS jobq
(
    id         bigserial primary key,
    name       text                     NOT NULL,
    username   text                     NOT NULL,
    uuid       uuid                     NOT NULL,
    params     jsonb                    NOT NULL,
    priority   priority_enum            NOT NULL,
    status     status_enum              NOT NULL,
    started_at timestamp with time zone NOT NULL DEFAULT now(),
    duration   double precision,
    error      text
);

CREATE INDEX IF NOT EXISTS status_idx ON jobq (status);

CREATE INDEX IF NOT EXISTS priority_idx ON jobq (priority);