/**
* Table for Bot aliases
*/
CREATE TABLE bot_aliases(
    -- Permanent User ID
    bot_id VARCHAR(1024) NOT NULL,

    -- Alias Object ID (ie, the URL)
    -- A remote bot can be aliased at most once
    alias_id VARCHAR(1024) UNIQUE NOT NULL,

    FOREIGN KEY (bot_id) REFERENCES bot_users(id)
);
