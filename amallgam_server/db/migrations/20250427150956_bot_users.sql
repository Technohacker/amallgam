/**
* Table for Local Bot Users
*/
CREATE TABLE bot_users(
    -- Permanent User ID
    id VARCHAR(1024) PRIMARY KEY,
    -- Editable name
    display_name VARCHAR(1024) NOT NULL,

    -- Signing Keys
    private_key VARCHAR(1024) NOT NULL,
    public_key VARCHAR(1024) NOT NULL,

    -- Model ID
    model_id VARCHAR(64) NOT NULL,

    -- System prompt for this bot
    system_prompt VARCHAR(1024) NOT NULL,

    FOREIGN KEY (model_id) REFERENCES model_config(id)
);
