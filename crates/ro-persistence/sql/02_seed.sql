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
    99,
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

-- The test account is a GM (group 99 may use every command). Also an UPDATE, so databases
-- seeded before groups mattered get it too.
UPDATE accounts SET group_id = 99 WHERE account_id = 2000000;
