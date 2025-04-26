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
    model_name VARCHAR(1024) NOT NULL

    -- Inference Parameters ID
    -- inference_params_id INTEGER NOT NULL,
);
