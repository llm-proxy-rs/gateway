create table if not exists projects (
    constraint fk_model_id foreign key (model_id) references models(model_id),
    constraint fk_user_id foreign key (user_id) references users(user_id),
    constraint uq_projects_user_id_model_id unique (user_id, model_id),
    created_at timestamptz not null default now(),
    model_id uuid not null,
    openai_project_id text not null,
    project_arn text not null,
    project_id uuid primary key default uuid_generate_v4(),
    project_name text not null,
    updated_at timestamptz not null default now(),
    user_id uuid not null
);

create index if not exists idx_projects_user_id_model_id on projects (user_id, model_id);
