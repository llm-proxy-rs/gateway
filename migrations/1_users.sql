create table if not exists users (
    created_at timestamptz not null default now(),
    email varchar(255) not null unique,
    updated_at timestamptz not null default now(),
    usage_record boolean not null default false,
    user_id uuid primary key default uuid_generate_v4(),
    user_role varchar(255) not null default 'user'
);
