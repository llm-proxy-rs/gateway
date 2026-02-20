-- Remove duplicate inference profiles, keeping the earliest one per (user_id, model_id)
DELETE FROM inference_profiles
WHERE inference_profile_id NOT IN (
    SELECT DISTINCT ON (user_id, model_id) inference_profile_id
    FROM inference_profiles
    ORDER BY user_id, model_id, created_at ASC
);

-- Drop the old non-unique index (replaced by the unique constraint below)
DROP INDEX IF EXISTS idx_inference_profiles_user_id_model_id;

-- Add unique constraint on (user_id, model_id)
ALTER TABLE inference_profiles ADD CONSTRAINT uq_inference_profiles_user_id_model_id UNIQUE (user_id, model_id);
