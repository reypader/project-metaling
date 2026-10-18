-- Seed data for Project Metaling development.
-- Inserts the baseline development account with zero characters.

INSERT INTO accounts (
    account_id,
    username,
    password_hash,
    sex,
    email,
    group_id,
    state,
    unban_time,
    expiration_time
) VALUES (
    2000000,
    'test',
    'password',
    1,
    'test@example.com',
    0,
    0,
    NULL,
    NULL
) ON CONFLICT (account_id) DO NOTHING;

-- Synchronize sequence state so subsequent generated accounts start at 2000001.
SELECT setval(
    pg_get_serial_sequence('accounts', 'account_id'),
    GREATEST((SELECT COALESCE(MAX(account_id), 2000000) FROM accounts), 2000000),
    true
);
