create table if not exists api_keys (
    api_key varchar(255) not null unique,
    api_key_id uuid primary key default uuid_generate_v4(),
    constraint fk_user_id foreign key (user_id) references users(user_id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    user_id uuid not null references users(user_id)
);

create index if not exists idx_api_keys_user_id on api_keys (user_id);
