create table if not exists inference_profiles (
    constraint fk_model_id foreign key (model_id) references models(model_id),
    constraint fk_user_id foreign key (user_id) references users(user_id),
    created_at timestamptz not null default now(),
    inference_profile_arn text not null,
    inference_profile_id uuid primary key default uuid_generate_v4(),
    inference_profile_name text not null,
    model_id uuid not null,
    updated_at timestamptz not null default now(),
    user_id uuid not null
);

create index if not exists idx_inference_profiles_user_id_model_id on inference_profiles (user_id, model_id);
