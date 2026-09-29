-- ADR-016 anchor E2E seed (#912) — DEV/TEST ONLY.
--
-- Creates the E2E team + an active API key whose raw bearer value is
-- `rgx_dev_sk_e2e_secret_value` (scopes: write_audit + read_*). The
-- `key_hash` below is Argon2id (the enterprise verifies with
-- `Argon2::default()`), generated for that exact raw key. Never use in prod.

INSERT INTO team_management.teams (id, name, slug, plan)
VALUES ('00000000-0000-0000-0000-000000000001', 'E2E', 'e2e', 'enterprise')
ON CONFLICT (id) DO NOTHING;

DELETE FROM auth.api_keys WHERE name = 'e2e-anchor';
INSERT INTO auth.api_keys
    (id, key_prefix, key_hash, key_hint, team_id, scopes, name, status, created_by)
VALUES (
    gen_random_uuid(),
    'rgx_dev_sk_',
    '$argon2id$v=19$m=19456,t=2,p=1$cmlnb3JpeC1lMmUtc2FsdA$cvb9+1WHMzZOCizStQ1Ccp5LVnLtdelv9tmdY538R3k',
    'alue',
    '00000000-0000-0000-0000-000000000001',
    '["write_audit","read_executions","read_policies"]'::jsonb,
    'e2e-anchor',
    'active',
    '00000000-0000-0000-0000-000000000001'
);
