-- Simbank Database Schema
-- Version: 1.0
-- Date: 2026-01-25

-- Wallet types enum table
CREATE TABLE wallet_types (
    code TEXT PRIMARY KEY,      -- 'spot', 'margin', 'futures', 'funding', 'earn'
    name TEXT NOT NULL,
    description TEXT
);

-- Currencies with dynamic decimals
CREATE TABLE currencies (
    code TEXT PRIMARY KEY,      -- 'USD', 'VND', 'BTC', 'ETH', 'USDT'
    name TEXT NOT NULL,
    decimals INTEGER NOT NULL,  -- 2, 0, 8, 18, 6
    symbol TEXT
);

-- Person types
CREATE TABLE persons (
    id TEXT PRIMARY KEY,        -- 'CUST_001', 'EMP_001'
    person_type TEXT NOT NULL,  -- 'customer', 'employee', 'shareholder', 'manager', 'auditor'
    name TEXT NOT NULL,
    email TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Accounts (1:1 with Person)
CREATE TABLE accounts (
    id TEXT PRIMARY KEY,        -- 'ACC_001'
    person_id TEXT NOT NULL UNIQUE,
    status TEXT DEFAULT 'active',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (person_id) REFERENCES persons(id)
);

-- Wallets (mỗi account có nhiều wallets)
CREATE TABLE wallets (
    id TEXT PRIMARY KEY,        -- 'WAL_001'
    account_id TEXT NOT NULL,
    wallet_type TEXT NOT NULL,  -- 'spot', 'funding'
    status TEXT DEFAULT 'active',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(account_id, wallet_type),
    FOREIGN KEY (account_id) REFERENCES accounts(id),
    FOREIGN KEY (wallet_type) REFERENCES wallet_types(code)
);

-- Balances (mỗi wallet có nhiều currencies)
CREATE TABLE balances (
    wallet_id TEXT NOT NULL,
    currency_code TEXT NOT NULL,
    available TEXT NOT NULL DEFAULT '0',    -- Decimal as TEXT
    locked TEXT NOT NULL DEFAULT '0',       -- Phase 2 logic
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (wallet_id, currency_code),
    FOREIGN KEY (wallet_id) REFERENCES wallets(id),
    FOREIGN KEY (currency_code) REFERENCES currencies(code)
);

-- Transactions (immutable ledger)
CREATE TABLE transactions (
    id TEXT PRIMARY KEY,        -- 'TXN_001'
    account_id TEXT NOT NULL,
    wallet_id TEXT NOT NULL,
    tx_type TEXT NOT NULL,      -- 'deposit', 'withdrawal', 'internal_transfer', 'trade'
    amount TEXT NOT NULL,       -- Decimal as TEXT
    currency_code TEXT NOT NULL,
    description TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (account_id) REFERENCES accounts(id),
    FOREIGN KEY (wallet_id) REFERENCES wallets(id)
);

-- Seed data: Wallet types
INSERT INTO wallet_types VALUES
    ('spot', 'Spot Wallet', 'For trading'),
    ('funding', 'Funding Wallet', 'For deposit/withdraw'),
    ('margin', 'Margin Wallet', 'For margin trading'),
    ('futures', 'Futures Wallet', 'For futures contracts'),
    ('earn', 'Earn Wallet', 'For staking/savings');

-- Seed data: Currencies (fiat + crypto)
INSERT INTO currencies VALUES
    ('VND', 'Vietnamese Dong', 0, '₫'),
    ('USD', 'US Dollar', 2, '$'),
    ('USDT', 'Tether', 6, '₮'),
    ('USDC', 'USD Coin', 6, '$'),
    ('BTC', 'Bitcoin', 8, '₿'),
    ('ETH', 'Ethereum', 18, 'Ξ'),
    ('DOGE', 'Dogecoin', 8, 'Ð'),
    ('ADA', 'Cardano', 6, '₳');
