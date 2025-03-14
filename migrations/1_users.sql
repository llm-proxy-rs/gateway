create table if not exists users (
    created_at timestamptz not null default now(),
    email varchar(255) not null unique,
    total_spent double precision not null default 0,
    total_tokens bigint not null default 0,
    updated_at timestamptz not null default now(),
    user_id uuid primary key default uuid_generate_v4(),
    user_role varchar(255) not null default 'user'
);
