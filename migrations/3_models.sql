create table if not exists models (
    created_at timestamptz not null default now(),
    input_price_per_token double precision,
    model_id uuid primary key default uuid_generate_v4(),
    model_name varchar(255) not null unique,
    output_price_per_token double precision,
    updated_at timestamptz not null default now()
);
