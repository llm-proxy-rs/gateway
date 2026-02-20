UPDATE models
SET is_disabled = true
WHERE model_name IN (
    'us.anthropic.claude-opus-4-5-20251101-v1:0',
    'us.anthropic.claude-sonnet-4-5-20250929-v1:0'
);

INSERT INTO models (model_name, protected)
VALUES
    ('global.anthropic.claude-opus-4-6-v1', true),
    ('global.anthropic.claude-sonnet-4-6', true);
