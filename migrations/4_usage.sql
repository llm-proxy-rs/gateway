create table if not exists usage (
    api_key_id uuid not null,
    constraint fk_api_key_id foreign key (api_key_id) references api_keys(api_key_id),
    constraint fk_model_id foreign key (model_id) references models(model_id),
    constraint fk_user_id foreign key (user_id) references users(user_id),
    created_at timestamptz not null default now(),
    model_id uuid not null,
    total_input_cost numeric(20, 10) not null,
    total_input_tokens bigint not null,
    total_output_cost numeric(20, 10) not null,
    total_output_tokens bigint not null,
    updated_at timestamptz not null default now(),
    usage_id uuid primary key default uuid_generate_v4(),
    user_id uuid not null
);

create index if not exists idx_usage_user_id on usage (user_id);
