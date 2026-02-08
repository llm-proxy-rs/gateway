create table if not exists users (
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    user_email varchar(255) not null unique,
    user_id uuid primary key default uuid_generate_v4()
);
