create table if not exists models (
    created_at timestamptz not null default now(),
    model_id uuid primary key default uuid_generate_v4(),
    model_name varchar(255) not null unique,
    updated_at timestamptz not null default now()
);
