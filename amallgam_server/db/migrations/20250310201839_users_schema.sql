/**
* Table for Local Bot Users
*/
CREATE TABLE bot_users(
    -- Permanent User ID
    user_id VARCHAR(1024) PRIMARY KEY,
    -- Editable name
    display_name VARCHAR(1024) NOT NULL,

    -- Signing Keys
    private_key VARCHAR(1024) NOT NULL,
    public_key VARCHAR(1024) NOT NULL,

    -- Model Name
    model_name VARCHAR(1024) NOT NULL,

    -- System Prompt
    system_prompt VARCHAR(1024) NOT NULL
);
