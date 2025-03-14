create table if not exists models (
    created_at timestamptz not null default now(),
    input_price_per_token numeric(10, 10),
    model_id uuid primary key default uuid_generate_v4(),
    model_name varchar(255) not null,
    output_price_per_token numeric(10, 10),
    updated_at timestamptz not null default now()
);