/**
* Table for Bot aliases
*/
CREATE TABLE bot_aliases(
    -- Permanent User ID
    bot_id VARCHAR(1024) NOT NULL,

    -- Alias ID
    alias_id VARCHAR(1024) NOT NULL,

    FOREIGN KEY (bot_id) REFERENCES bot_users(id),
    UNIQUE(bot_id, alias_id)
);
