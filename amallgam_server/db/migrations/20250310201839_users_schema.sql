/**
* Table for Federated User Data
*/
CREATE TABLE users(
    -- Federation ID, URL
    fed_id VARCHAR(1024) PRIMARY KEY,

    -- Permanent User ID
    preferred_username VARCHAR(1024) NOT NULL,
    -- Editable name
    name VARCHAR(1024) NOT NULL,

    -- Links for submitting Actions
    inbox VARCHAR(1024) NOT NULL,
    outbox VARCHAR(1024) NOT NULL,

    -- Public Key
    public_key VARCHAR(1024) NOT NULL,

    -- Shared Inbox, optional
    shared_inbox VARCHAR(1024)
);

/**
* Table with extra data for local Bot users
*/
CREATE TABLE bot_users(
    -- Federation ID, URL
    fed_id VARCHAR(1024) PRIMARY KEY,

    -- Private Key for signing
    private_key VARCHAR(1024) NOT NULL,

    -- TODO: Use these once the tables are ready

    -- Model ID
    -- model_id INTEGER NOT NULL,

    -- Inference Parameters ID
    -- inference_params_id INTEGER NOT NULL,

    FOREIGN KEY(fed_id) REFERENCES users(fed_id)
);

-- TODO: Remove this temp user
INSERT INTO users (
    fed_id,
    preferred_username,
    name,
    inbox,
    outbox,
    public_key
) VALUES (  
    "https://amallgam.docker/user/abc",
    "abc",
    "Abc",
    "https://amallgam.docker/user/abc/inbox",
    "https://amallgam.docker/user/abc/outbox",
    "{}"
);
